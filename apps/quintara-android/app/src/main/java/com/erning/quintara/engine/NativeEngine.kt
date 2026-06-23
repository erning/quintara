package com.erning.quintara.engine

object NativeEngine {
    private val loaded: Boolean = runCatching {
        System.loadLibrary("quintara_android_jni")
        true
    }.getOrDefault(false)

    fun isLoaded(): Boolean = loaded

    fun isRapfiAvailable(): Boolean {
        if (!loaded) {
            return false
        }
        return runCatching { nativeRapfiAvailable() }.getOrDefault(false)
    }

    fun createSession(config: EngineConfig): NativeSession {
        check(loaded) { "libquintara_android_jni.so is not loaded" }
        val handle = nativeCreateSession(config.toJson().toString())
        check(handle != 0L) { "native session was not created" }
        return NativeSession(handle)
    }

    private external fun nativeRapfiAvailable(): Boolean
    private external fun nativeCreateSession(configJson: String): Long
    private external fun nativeTick(handle: Long, inputJson: String?): String
    private external fun nativeSnapshot(handle: Long): String
    private external fun nativeExportPsq(handle: Long): String
    private external fun nativeDisposeSession(handle: Long)

    class NativeSession internal constructor(private var handle: Long) : AutoCloseable {
        fun tick(input: EngineInput? = null): EngineStep {
            ensureOpen()
            return EngineJson.parseStep(nativeTick(handle, input?.toJson()?.toString()))
        }

        fun snapshot(): EngineSnapshot {
            ensureOpen()
            return EngineJson.parseSnapshot(nativeSnapshot(handle))
        }

        fun exportPsq(): String {
            ensureOpen()
            return nativeExportPsq(handle)
        }

        override fun close() {
            val current = handle
            if (current != 0L) {
                nativeDisposeSession(current)
                handle = 0L
            }
        }

        private fun ensureOpen() {
            check(handle != 0L) { "native session is closed" }
        }
    }
}
