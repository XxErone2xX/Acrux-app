import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.acrux.next"
    sourceSets.getByName("main").assets.srcDir("../../../../android/app/src/main/assets")
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.acrux.next"
        minSdk = 26
        targetSdk = 35
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    // 알파 서명: CI 가 ACRUX_ALPHA_KEYSTORE 를 줌 (비밀값 키 · 없으면 CI 가 캐시해 둔 디버그 키) → 디버그 · 릴리스 둘 다 같은 키
    signingConfigs {
        val ks = System.getenv("ACRUX_ALPHA_KEYSTORE")
        if (ks != null && file(ks).exists()) {
            create("alpha") {
                storeFile = file(ks)
                storePassword = System.getenv("ACRUX_ALPHA_KEYSTORE_PASS")
                keyAlias = System.getenv("ACRUX_ALPHA_KEY_ALIAS") ?: "acrux"
                keyPassword = System.getenv("ACRUX_ALPHA_KEYSTORE_PASS")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            signingConfigs.findByName("alpha")?.let { signingConfig = it }
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            signingConfigs.findByName("alpha")?.let { signingConfig = it }
            isMinifyEnabled = false   // 알파: 줄이기 끔 (libadb · conscrypt 를 리플렉션으로 씀 → 줄이면 깨질 수 있음)
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    packaging {
        resources.excludes.addAll(listOf("META-INF/versions/9/OSGI-INF/MANIFEST.MF", "META-INF/DEPENDENCIES", "META-INF/LICENSE*", "META-INF/NOTICE*"))
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("com.github.MuntashirAkon:libadb-android:3.1.1")
    implementation("com.github.MuntashirAkon:sun-security-android:1.1")
    implementation("org.conscrypt:conscrypt-android:2.5.3")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))
