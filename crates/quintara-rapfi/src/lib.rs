//! Library-style Rapfi integration boundary.
//!
//! Android uses Rapfi as a native shared library, not as a `pbrain-rapfi`
//! executable. This crate keeps that boundary behind the existing
//! [`quintara_bot::MoveSource`] port so the rest of quintara can treat Rapfi
//! like any other local bot.

#![allow(unsafe_code)] // NOTE: The Android backend talks to Rapfi through a C ABI.
#![allow(clippy::module_name_repetitions)]

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, Position, TurnContext};

/// Files needed by the Rapfi native library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RapfiConfig {
    /// Path to Rapfi's `config.toml`.
    pub config_path: PathBuf,
    /// Directory containing Rapfi's classical and NNUE weight files.
    pub weights_dir: PathBuf,
    /// Per-move thinking budget used when the turn context does not provide one.
    pub default_thinking_time: Duration,
}

impl RapfiConfig {
    /// Builds a config from the directory where Android copied Rapfi assets.
    #[must_use]
    pub fn from_asset_dir(asset_dir: impl Into<PathBuf>, default_thinking_time: Duration) -> Self {
        let weights_dir = asset_dir.into();
        Self {
            config_path: weights_dir.join("config.toml"),
            weights_dir,
            default_thinking_time,
        }
    }
}

/// Rapfi integration errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RapfiError {
    /// The native Rapfi library has not been linked for this target yet.
    NativeLibraryUnavailable,
    /// The provided config or weight path is not usable.
    MissingAssets { detail: String },
    /// The native backend reported an initialization or search failure.
    Native { detail: String },
}

impl fmt::Display for RapfiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeLibraryUnavailable => {
                f.write_str("Rapfi native library is unavailable for this target")
            }
            Self::MissingAssets { detail } => write!(f, "Rapfi assets are missing: {detail}"),
            Self::Native { detail } => write!(f, "Rapfi native error: {detail}"),
        }
    }
}

impl std::error::Error for RapfiError {}

/// A Rapfi-backed local bot.
pub struct RapfiMoveSource {
    backend: native::NativeBackend,
}

impl RapfiMoveSource {
    /// Creates a Rapfi move source backed by the Android native library.
    ///
    /// # Errors
    /// Returns an error when this target cannot load Rapfi, or when the
    /// provided config and weights cannot be opened by the native backend.
    pub fn new(config: &RapfiConfig) -> Result<Self, RapfiError> {
        if !config.config_path.is_file() {
            return Err(RapfiError::MissingAssets {
                detail: config.config_path.display().to_string(),
            });
        }
        if !config.weights_dir.is_dir() {
            return Err(RapfiError::MissingAssets {
                detail: config.weights_dir.display().to_string(),
            });
        }
        native::NativeBackend::new(config).map(|backend| Self { backend })
    }

    /// Reports whether this build can construct a real Rapfi backend.
    #[must_use]
    pub fn is_available() -> bool {
        native::is_available()
    }
}

impl MoveSource for RapfiMoveSource {
    fn next_move(&mut self, ctx: &TurnContext, stop: &StopFlag) -> Move {
        self.backend.next_move(ctx, stop)
    }
}

fn fallback_move(ctx: &TurnContext) -> Move {
    ctx.legal_moves
        .first()
        .copied()
        .unwrap_or_else(|| Move::Place(Position::new(0, 0)))
}

#[cfg(target_os = "android")]
fn thinking_time(ctx: &TurnContext, default: Duration) -> Duration {
    match (ctx.timeout_turn, ctx.time_left) {
        (Some(turn), Some(left)) => turn.min(left),
        (Some(turn), None) => turn,
        (None, Some(left)) => left,
        (None, None) => default,
    }
}

#[cfg(target_os = "android")]
mod native {
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::path::Path;
    use std::ptr::NonNull;
    use std::time::Duration;

    use quintara_bot::StopFlag;
    use quintara_model::{Move, TurnContext};

    use super::{fallback_move, thinking_time, RapfiConfig, RapfiError};

    const RTLD_NOW: c_int = 2;

    #[repr(C)]
    struct RapfiHandle {
        _private: [u8; 0],
    }

    type CreateFn = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut RapfiHandle;
    type DestroyFn = unsafe extern "C" fn(*mut RapfiHandle);
    type NewGameFn = unsafe extern "C" fn(*mut RapfiHandle, c_int, c_int) -> c_int;
    type SetPositionFn =
        unsafe extern "C" fn(*mut RapfiHandle, *const c_int, *const c_int, c_int) -> c_int;
    type ThinkFn = unsafe extern "C" fn(*mut RapfiHandle, c_int, *mut c_int, *mut c_int) -> c_int;
    type StopFn = unsafe extern "C" fn(*mut RapfiHandle);
    type LastErrorFn = unsafe extern "C" fn(*mut RapfiHandle) -> *const c_char;
    type IsAvailableFn = unsafe extern "C" fn() -> c_int;

    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn dlerror() -> *const c_char;
    }

    pub(super) struct NativeBackend {
        handle: NonNull<RapfiHandle>,
        api: RapfiApi,
        default_thinking_time: Duration,
    }

    // SAFETY: Rapfi's public C ABI is serialized by the C++ wrapper's mutex, and
    // each backend owns one opaque native handle that is destroyed on drop.
    unsafe impl Send for NativeBackend {}

    impl NativeBackend {
        pub(super) fn new(config: &RapfiConfig) -> Result<Self, RapfiError> {
            let api = RapfiApi::load()?;
            let config_path = path_cstring(&config.config_path)?;
            let weights_dir = path_cstring(&config.weights_dir)?;
            let handle = unsafe {
                // SAFETY: The C strings live for the duration of this call, and
                // Rapfi copies the paths into its global config state.
                (api.create)(config_path.as_ptr(), weights_dir.as_ptr())
            };
            let handle = NonNull::new(handle).ok_or_else(|| RapfiError::Native {
                detail: "rapfi_create returned null".to_string(),
            })?;
            let backend = Self {
                handle,
                api,
                default_thinking_time: config.default_thinking_time,
            };
            if let Some(message) = backend.last_error().filter(|message| !message.is_empty()) {
                return Err(RapfiError::Native { detail: message });
            }
            Ok(backend)
        }

        pub(super) fn next_move(&mut self, ctx: &TurnContext, stop: &StopFlag) -> Move {
            if stop.should_stop() {
                self.stop();
                return fallback_move(ctx);
            }

            let Some(board_size) = ctx.board.square_size().map(c_int::from) else {
                return fallback_move(ctx);
            };
            let rule = c_int::from(ctx.rule_set.gomocup_rule_code().unwrap_or(0));
            let (xs, ys) = move_history_xy(ctx);
            let Ok(move_count) = c_int::try_from(xs.len()) else {
                return fallback_move(ctx);
            };
            let time_ms = duration_to_c_millis(thinking_time(ctx, self.default_thinking_time));

            let mut out_x: c_int = 0;
            let mut out_y: c_int = 0;
            let ok = unsafe {
                // SAFETY: `self.handle` is owned by this backend. The coordinate
                // arrays remain live through the calls, and output pointers are valid.
                (self.api.new_game)(self.handle.as_ptr(), board_size, rule) == 0
                    && (self.api.set_position)(
                        self.handle.as_ptr(),
                        xs.as_ptr(),
                        ys.as_ptr(),
                        move_count,
                    ) == 0
                    && (self.api.think)(self.handle.as_ptr(), time_ms, &mut out_x, &mut out_y) == 0
            };
            if !ok {
                return fallback_move(ctx);
            }

            let (Ok(row), Ok(col)) = (u8::try_from(out_y), u8::try_from(out_x)) else {
                return fallback_move(ctx);
            };
            let candidate = Move::Place(quintara_model::Position::new(row, col));
            if ctx.legal_moves.contains(&candidate) {
                candidate
            } else {
                fallback_move(ctx)
            }
        }

        fn stop(&self) {
            unsafe {
                // SAFETY: `self.handle` remains valid for the lifetime of the backend.
                (self.api.stop)(self.handle.as_ptr());
            }
        }

        fn last_error(&self) -> Option<String> {
            let message = unsafe {
                // SAFETY: Rapfi returns either null or a pointer to handle-owned
                // storage, which remains valid while `self` is alive.
                (self.api.last_error)(self.handle.as_ptr())
            };
            if message.is_null() {
                None
            } else {
                Some(
                    unsafe { CStr::from_ptr(message) }
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        }
    }

    impl Drop for NativeBackend {
        fn drop(&mut self) {
            unsafe {
                // SAFETY: `self.handle` was returned by `rapfi_create` and has
                // not been destroyed elsewhere.
                (self.api.destroy)(self.handle.as_ptr());
            }
        }
    }

    pub(super) fn is_available() -> bool {
        RapfiApi::load().is_ok_and(|api| unsafe {
            // SAFETY: The function pointer was resolved from `librapfi.so`.
            (api.is_available)() != 0
        })
    }

    struct RapfiApi {
        create: CreateFn,
        destroy: DestroyFn,
        new_game: NewGameFn,
        set_position: SetPositionFn,
        think: ThinkFn,
        stop: StopFn,
        last_error: LastErrorFn,
        is_available: IsAvailableFn,
        _library: DynamicLibrary,
    }

    impl RapfiApi {
        fn load() -> Result<Self, RapfiError> {
            let library = DynamicLibrary::open("librapfi.so")?;
            let create = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, CreateFn>(library.symbol(c"rapfi_create")?)
            };
            let destroy = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, DestroyFn>(library.symbol(c"rapfi_destroy")?)
            };
            let new_game = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, NewGameFn>(library.symbol(c"rapfi_new_game")?)
            };
            let set_position = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, SetPositionFn>(
                    library.symbol(c"rapfi_set_position")?,
                )
            };
            let think = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, ThinkFn>(library.symbol(c"rapfi_think")?)
            };
            let stop = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, StopFn>(library.symbol(c"rapfi_stop")?)
            };
            let last_error = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, LastErrorFn>(
                    library.symbol(c"rapfi_last_error")?,
                )
            };
            let is_available = unsafe {
                // SAFETY: The symbol name and type match `rapfi_c_api.h`.
                std::mem::transmute::<*mut c_void, IsAvailableFn>(
                    library.symbol(c"rapfi_is_available")?,
                )
            };
            Ok(Self {
                create,
                destroy,
                new_game,
                set_position,
                think,
                stop,
                last_error,
                is_available,
                _library: library,
            })
        }
    }

    struct DynamicLibrary {
        handle: NonNull<c_void>,
    }

    // SAFETY: The handle is only used to resolve immutable function pointers and
    // is closed after all backend-owned Rapfi handles have been destroyed.
    unsafe impl Send for DynamicLibrary {}

    impl DynamicLibrary {
        fn open(name: &str) -> Result<Self, RapfiError> {
            let name = CString::new(name).map_err(|e| RapfiError::Native {
                detail: e.to_string(),
            })?;
            let handle = unsafe {
                // SAFETY: `name` is a valid null-terminated C string.
                dlopen(name.as_ptr(), RTLD_NOW)
            };
            NonNull::new(handle)
                .map(|handle| Self { handle })
                .ok_or_else(|| dl_error("dlopen librapfi.so"))
        }

        fn symbol(&self, name: &'static CStr) -> Result<*mut c_void, RapfiError> {
            let symbol = unsafe {
                // SAFETY: The library handle is valid and `name` is null-terminated.
                dlsym(self.handle.as_ptr(), name.as_ptr())
            };
            if symbol.is_null() {
                Err(dl_error(&format!("dlsym {}", name.to_string_lossy())))
            } else {
                Ok(symbol)
            }
        }
    }

    impl Drop for DynamicLibrary {
        fn drop(&mut self) {
            unsafe {
                // SAFETY: The handle was returned by `dlopen`.
                dlclose(self.handle.as_ptr());
            }
        }
    }

    fn path_cstring(path: &Path) -> Result<CString, RapfiError> {
        CString::new(path.to_string_lossy().into_owned()).map_err(|e| RapfiError::Native {
            detail: e.to_string(),
        })
    }

    fn move_history_xy(ctx: &TurnContext) -> (Vec<c_int>, Vec<c_int>) {
        let mut xs = Vec::with_capacity(ctx.move_history.len());
        let mut ys = Vec::with_capacity(ctx.move_history.len());
        for mv in &ctx.move_history {
            let pos = mv.position();
            xs.push(c_int::from(pos.col));
            ys.push(c_int::from(pos.row));
        }
        (xs, ys)
    }

    fn duration_to_c_millis(duration: Duration) -> c_int {
        c_int::try_from(duration.as_millis().max(1)).unwrap_or(c_int::MAX)
    }

    fn dl_error(context: &str) -> RapfiError {
        let message = unsafe {
            // SAFETY: `dlerror` returns a process-local error string or null.
            let error = dlerror();
            if error.is_null() {
                context.to_string()
            } else {
                format!("{context}: {}", CStr::from_ptr(error).to_string_lossy())
            }
        };
        RapfiError::Native { detail: message }
    }
}

#[cfg(not(target_os = "android"))]
mod native {
    use quintara_bot::StopFlag;
    use quintara_model::{Move, TurnContext};

    use super::{fallback_move, RapfiConfig, RapfiError};

    pub(super) struct NativeBackend;

    impl NativeBackend {
        pub(super) fn new(_config: &RapfiConfig) -> Result<Self, RapfiError> {
            Err(RapfiError::NativeLibraryUnavailable)
        }

        pub(super) fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
            let _ = self;
            fallback_move(ctx)
        }
    }

    pub(super) const fn is_available() -> bool {
        false
    }
}
