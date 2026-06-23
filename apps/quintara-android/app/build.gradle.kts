plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val androidNdkVersion = "29.0.14206865"
val rustTarget = "aarch64-linux-android"
val rustAbi = "arm64-v8a"
val repoRoot = layout.projectDirectory.asFile.resolve("../../..").canonicalFile
val rapfiAssetSource = repoRoot.resolve("bots/rapfi/build")
val generatedRapfiAssets = layout.buildDirectory.dir("generated/assets/rapfi")
val androidHome = providers.environmentVariable("ANDROID_HOME")
    .orElse(providers.provider { "${System.getProperty("user.home")}/Library/Android/sdk" })
val ndkToolchain = androidHome.map {
    "$it/ndk/$androidNdkVersion/toolchains/llvm/prebuilt/darwin-x86_64/bin"
}

android {
    namespace = "com.erning.quintara"
    compileSdk = 36
    ndkVersion = androidNdkVersion

    defaultConfig {
        applicationId = "com.erning.quintara"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1"
        ndk {
            abiFilters += rustAbi
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    externalNativeBuild {
        cmake {
            path = file("../native/rapfi/CMakeLists.txt")
            version = "3.31.6"
        }
    }

    sourceSets["main"].assets.srcDir(generatedRapfiAssets)
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.foundation:foundation:1.8.3")
    implementation("androidx.compose.material3:material3:1.3.2")
    implementation("androidx.compose.ui:ui:1.8.3")
    implementation("androidx.compose.ui:ui-tooling-preview:1.8.3")
    debugImplementation("androidx.compose.ui:ui-tooling:1.8.3")
}

val buildRustArm64 by tasks.registering(Exec::class) {
    workingDir = repoRoot
    commandLine(
        "cargo",
        "build",
        "-p",
        "quintara-android-jni",
        "--target",
        rustTarget,
        "--release",
    )
    doFirst {
        val toolchain = ndkToolchain.get()
        environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", "$toolchain/aarch64-linux-android26-clang")
        environment("AR_aarch64_linux_android", "$toolchain/llvm-ar")
    }
}

val copyRustArm64 by tasks.registering(Copy::class) {
    dependsOn(buildRustArm64)
    from(repoRoot.resolve("target/$rustTarget/release/libquintara_android_jni.so"))
    into(layout.projectDirectory.dir("src/main/jniLibs/$rustAbi"))
}

val copyRapfiAssets by tasks.registering(Copy::class) {
    from(rapfiAssetSource) {
        include(
            "config.toml",
            "model*.bin",
            "mix9svq*.bin.lz4",
        )
    }
    into(generatedRapfiAssets.map { it.dir("rapfi") })
    onlyIf { rapfiAssetSource.resolve("config.toml").isFile }
}

tasks.named("preBuild") {
    dependsOn(copyRustArm64, copyRapfiAssets)
}
