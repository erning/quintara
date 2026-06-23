# quintara Android

Phone-only Android app for quintara.

## Shape

- UI: Kotlin + Jetpack Compose.
- Engine boundary: Kotlin calls `libquintara_android_jni.so`.
- Game logic: Rust keeps using `quintara-mobile` and `MatchConductor`.
- Rapfi: packaged as `librapfi.so` and called as a native library through the C ABI in
  `native/rapfi/rapfi_c_api.h`.

The Android app does **not** execute `pbrain-rapfi` as a child process.

## Rapfi Status

`native/rapfi/rapfi_android.cpp` wraps the upstream Rapfi C++ sources behind
`rapfi_c_api.h`. Gradle packages `librapfi.so`; Rust loads it at runtime and uses it
for the Master difficulty.

Rapfi assets are copied from `bots/rapfi/build` into Android assets, then unpacked into
an app-private directory before creating the engine handle:

```text
config.toml
model*.bin
mix9svq*.bin.lz4
```

## Build

This directory is a standalone Gradle project:

```sh
cd apps/quintara-android
./gradlew :app:assembleDebug
```

If the Android SDK is not discoverable, either open the project in Android Studio so it
creates `local.properties`, or run with `ANDROID_HOME` pointing at the SDK.

The Gradle build regenerates `app/src/main/jniLibs/arm64-v8a/libquintara_android_jni.so`
from the Rust crate; generated JNI outputs are ignored by git.
