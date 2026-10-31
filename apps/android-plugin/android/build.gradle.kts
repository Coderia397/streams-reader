// Streams Reader Android layer (Tauri 2 mobile plugin, Kotlin).
// Versions verified 2026-10-30 against Maven metadata and the AAR manifests (notes/18):
//   Media3 1.11.1 (stable, 2026-09-10): minSdk 23, minCompileSdk 36. 1.12.0-alpha02 exists; do not use alphas.
//   androidx.core 1.18.0: last release that still compiles against SDK 36 (1.19.1 needs compileSdk 37).
// Layout copied from the official tauri-plugin-dialog 2.8.4 Android module.
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

val media3 = "1.11.1"

android {
    namespace = "watch.streams.reader.media"
    compileSdk = 36

    defaultConfig {
        // Tauri 2's default Android minSdk (tauri-utils `default_min_sdk_version`); Media3 1.11 needs 23.
        minSdk = 24
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_17
    }
}

dependencies {
    implementation("androidx.media3:media3-exoplayer:$media3")
    implementation("androidx.media3:media3-session:$media3")
    implementation("androidx.media3:media3-common:$media3")
    implementation("androidx.core:core-ktx:1.18.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    // MediaController futures use addListener + main executor; add kotlinx-coroutines-guava 1.11.0 only if
    // coroutines are introduced.
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    implementation(project(":tauri-android"))
}
