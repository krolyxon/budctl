import com.android.build.api.artifact.SingleArtifact

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// One version for everything (CLAUDE.md, releases): the cargo workspace's.
val workspace: File = rootDir.parentFile
val version: String = workspace.resolve("Cargo.toml").readLines()
    .first { it.startsWith("version = ") }
    .substringAfter('"').substringBefore('"')

android {
    namespace = "io.github.krolyxon.budctl"
    compileSdk = 37
    // For stripping the Rust library's debug symbols from the APK, and for cargo-ndk.
    ndkVersion = "30.0.16248370"

    defaultConfig {
        applicationId = "io.github.krolyxon.budctl"
        // Android 12: BLUETOOTH_CONNECT begins there (docs/android.md#bluetooth).
        minSdk = 31
        targetSdk = 37
        versionName = version
        versionCode = version.split('.').fold(0) { code, part -> code * 100 + part.toInt() }
    }

    buildTypes {
        // Only where the Rust library is built for: the phone, and the emulator in debug builds.
        debug {
            ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        }
        release {
            ndk { abiFilters += listOf("arm64-v8a") }
            // The Rust library calls Platform's methods by name, so nothing may rename them.
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        compose = true
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
    testImplementation("junit:junit:4.13.2")
}

/** The Rust library (crates/budctl-android), built by cargo-ndk into [output]/<abi>/. */
abstract class CargoNdk : Exec() {
    @get:OutputDirectory
    abstract val output: DirectoryProperty
}

/** Rule 1 (CLAUDE.md): the app never gets network access, not even from a library's manifest. */
abstract class CheckNoInternet : DefaultTask() {
    @get:InputFile
    abstract val manifest: RegularFileProperty

    @TaskAction
    fun check() {
        if ("android.permission.INTERNET" in manifest.get().asFile.readText()) {
            throw GradleException("The merged manifest asks for INTERNET; budctl must not (CLAUDE.md rule 1).")
        }
    }
}

androidComponents {
    onVariants { variant ->
        val name = variant.name.replaceFirstChar { it.uppercase() }
        val release = variant.buildType == "release"
        // Debug builds also run on the emulator (x86_64); releases only on the phone.
        val abis = if (release) listOf("arm64-v8a") else listOf("arm64-v8a", "x86_64")
        val ndk = sdkComponents.ndkDirectory

        val cargo = tasks.register<CargoNdk>("cargoNdk$name") {
            workingDir = workspace
            executable = "cargo"
            // Cargo knows what to rebuild, so it always runs.
            outputs.upToDateWhen { false }
            argumentProviders.add {
                listOf("ndk", "--platform", "31") + abis.flatMap { listOf("--target", it) } +
                    listOf("--output-dir", output.get().asFile.path, "build", "-p", "budctl-android") +
                    if (release) listOf("--release") else emptyList()
            }
            doFirst { environment("ANDROID_NDK_HOME", ndk.get().asFile.path) }
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdk::output)

        val check = tasks.register<CheckNoInternet>("check${name}NoInternet") {
            manifest.set(variant.artifacts.get(SingleArtifact.MERGED_MANIFEST))
        }
        tasks.matching { it.name == "package$name" }.configureEach { dependsOn(check) }
    }
}
