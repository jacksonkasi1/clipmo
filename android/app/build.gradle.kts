plugins {
 id("com.android.application")
 kotlin("android")
}

android {
 namespace = "app.clipdeck.desktop"
 compileSdk = 35
 defaultConfig {
 applicationId = "app.clipdeck.desktop"
 minSdk = 29
 targetSdk = 35
 versionCode = 9
 versionName = "0.2.10"
 }
 signingConfigs {
 create("release") {
 storeFile = rootProject.file(System.getenv("ANDROID_KEYSTORE_PATH") ?: "debug.keystore")
 storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD") ?: "android"
 keyAlias = System.getenv("ANDROID_KEY_ALIAS") ?: "androiddebugkey"
 keyPassword = System.getenv("ANDROID_KEY_PASSWORD") ?: "android"
 }
 }
 buildTypes {
 release {
 isMinifyEnabled = true
 signingConfig = signingConfigs.getByName("release")
 proguardFiles(
 getDefaultProguardFile("proguard-android-optimize.txt"),
 "proguard-rules.pro"
 )
 }
 debug {
 applicationIdSuffix = ".debug"
 signingConfig = signingConfigs.getByName("release")
 }
 }
 compileOptions {
 sourceCompatibility = JavaVersion.VERSION_17
 targetCompatibility = JavaVersion.VERSION_17
 }
 kotlinOptions { jvmTarget = "17" }
 buildFeatures {
 compose = true
 viewBinding = false
 }
 composeOptions { kotlinCompilerExtensionVersion = "1.5.14" }
}

dependencies {
 implementation("androidx.core:core-ktx:1.15.0")
 implementation("androidx.activity:activity-compose:1.9.3")
 implementation(platform("androidx.compose:compose-bom:2024.06.00"))
 implementation("androidx.compose.foundation:foundation")
 implementation("androidx.compose.runtime:runtime")
 implementation("androidx.compose.ui:ui")
 implementation("androidx.compose.ui:ui-text-google-fonts")
 implementation("androidx.compose.ui:ui-tooling-preview")
 debugImplementation("androidx.compose.ui:ui-tooling")
 implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.8.7")
 implementation("androidx.lifecycle:lifecycle-livedata-ktx:2.8.7")
 implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
 implementation("androidx.datastore:datastore-preferences:1.1.1")
 implementation("androidx.compose.material:material")
 implementation("androidx.compose.material:material-icons-extended")
 implementation("com.fasterxml.jackson.core:jackson-databind:2.17.2")
 implementation("com.fasterxml.jackson.module:jackson-module-kotlin:2.17.2")
 testImplementation("junit:junit:4.13.2")
}
