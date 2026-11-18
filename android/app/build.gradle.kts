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
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    // Release signing reads the keystore from the environment so no secret is ever in the repo
    // (ANDROID-RELEASE.md). With the variables unset, `assembleRelease` builds an unsigned APK.
    val releaseKeystore: String? = System.getenv("ALARMANT_KEYSTORE_FILE")
    if (releaseKeystore != null) {
        signingConfigs {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = System.getenv("ALARMANT_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ALARMANT_KEY_ALIAS")
                keyPassword = System.getenv("ALARMANT_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (releaseKeystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
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

    testImplementation("junit:junit:4.13.2")
    // android.jar's org.json is a stub in JVM unit tests; the real library lets RegistrationTest parse JSON.
    testImplementation("org.json:json:20250517")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
}
