plugins {
    id("com.android.application")
    id("com.google.gms.google-services")
}

android {
    namespace = "com.thinkersjournal.alarmant"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.thinkersjournal.alarmant"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.8"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(platform("com.google.firebase:firebase-bom:35.1.0"))
    implementation("com.google.firebase:firebase-messaging")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("androidx.work:work-runtime-ktx:2.12.0")
    implementation("androidx.activity:activity-ktx:1.13.0")
}
