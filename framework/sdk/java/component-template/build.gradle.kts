/**
 * Guest Wasm is built with **Maven** + [com.fermyon teavm-wasi](https://github.com/fermyon/teavm-wasi)
 * (`pom.xml`), not the upstream `org.teavm` Gradle plugin (browser Wasm GC).
 *
 * Uses the **Maven Wrapper** (`mvnw` / `mvnw.cmd`) in this directory (no global `mvn`).
 * Requires a composite `:game:jar` for `sk.upjs.gdd:game`.
 *
 * TeaVM 0.2.x cannot parse JDK 21 bootstrap classes (major 65); `mavenPackage` runs Maven with **JDK 17** via toolchains.
 */

import java.net.URI
import org.gradle.jvm.toolchain.JavaLanguageVersion
import org.gradle.jvm.toolchain.JvmVendorSpec

plugins {
    java
}

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(17))
        vendor.set(JvmVendorSpec.ADOPTIUM)
    }
    sourceSets.named("main") {
        java.setSrcDirs(emptyList<String>())
        resources.setSrcDirs(emptyList<String>())
    }
}

val java17Launcher = javaToolchains.launcherFor {
    languageVersion.set(JavaLanguageVersion.of(17))
    vendor.set(JvmVendorSpec.ADOPTIUM)
}

tasks.named("jar") { enabled = false }

tasks.register<Exec>("generateWitBindings") {
    group = "codegen"
    description = "Regenerate wit/worlds/GameCore.java from test.wit (requires wit-bindgen-cli 0.40.x on PATH)."
    val wit = layout.projectDirectory.dir("../../..").file("test.wit").asFile
    val out = project.file("src/generated/java")
    inputs.file(wit)
    outputs.dir(out)
    doFirst {
        out.mkdirs()
    }
    commandLine(
        "wit-bindgen",
        "teavm-java",
        wit.absolutePath,
        "--world",
        "game-core",
        "--out-dir",
        out.absolutePath,
    )
}

val includedGame = gradle.includedBuilds.single()

tasks.register<Exec>("mavenPackage") {
    group = "build"
    description = "Run Maven package (TeaVM com.fermyon) to produce target/generated/wasm/…"
    dependsOn(includedGame.task(":jar"))
    inputs.file(layout.projectDirectory.file("pom.xml"))
    inputs.file(layout.projectDirectory.file("mvnw.cmd"))
    inputs.file(layout.projectDirectory.file("mvnw"))
    inputs.dir(layout.projectDirectory.dir(".mvn"))
    inputs.dir("src/main/java")
    val jarPath = includedGame.projectDir.resolve("build/libs/java-game-0.1.0.jar")
    doFirst {
        require(jarPath.isFile) { "Missing game JAR: ${jarPath.absolutePath}" }
    }
    val pom = layout.projectDirectory.file("pom.xml").asFile.absolutePath
    val mvnArgs = buildList {
        if (System.getProperty("os.name").lowercase().contains("windows")) {
            add("cmd")
            add("/c")
            add(layout.projectDirectory.file("mvnw.cmd").asFile.absolutePath)
        } else {
            add(layout.projectDirectory.file("mvnw").asFile.absolutePath)
        }
        addAll(
            listOf(
                "-q",
                "-f",
                pom,
                "package",
                "-DskipTests",
                "-Dgame.jar=${jarPath.absolutePath}",
            ),
        )
    }
    doFirst {
        val home = java17Launcher.get().metadata.installationPath.asFile.absolutePath
        environment("JAVA_HOME", home)
    }
    commandLine(mvnArgs)
    workingDir = layout.projectDirectory.asFile
}

val frameworkWit = layout.projectDirectory.file("../../../test.wit").asFile

tasks.register<Copy>("stageLogicCoreWasm") {
    group = "build"
    description = "Copy TeaVM core Wasm to build/out/logic-core.wasm."
    dependsOn("mavenPackage")
    from(layout.projectDirectory.dir("target/generated/wasm/teavm-wasm")) {
        include("*.wasm")
        rename { _ -> "logic-core.wasm" }
    }
    into(layout.buildDirectory.dir("out"))
    duplicatesStrategy = DuplicatesStrategy.FAIL
}

val wasiReactorAdapter = layout.projectDirectory.file("wasm/wasi_snapshot_preview1.reactor.wasm")
val teavmAsciiAdapter = layout.projectDirectory.file("wasm/teavm_ascii.wasm")
val wasiAdapterUrl =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v37.0.2/wasi_snapshot_preview1.reactor.wasm"

tasks.register("ensureWasiReactorAdapter") {
    group = "build"
    description = "Download WASI preview1 reactor adapter if wasm/wasi_snapshot_preview1.reactor.wasm is missing."
    val adapterFile = wasiReactorAdapter.asFile
    outputs.file(adapterFile)
    doLast {
        if (adapterFile.isFile) return@doLast
        adapterFile.parentFile.mkdirs()
        val conn = URI(wasiAdapterUrl).toURL().openConnection()
        conn.getInputStream().use { input ->
            adapterFile.outputStream().use { output -> input.copyTo(output) }
        }
    }
}

tasks.register<Exec>("embedLogicComponent") {
    group = "build"
    description = "Embed game-core WIT metadata into TeaVM core Wasm."
    dependsOn("stageLogicCoreWasm")
    val coreWasm = layout.buildDirectory.file("out/logic-core.wasm").get().asFile
    val embeddedWasm = layout.buildDirectory.file("out/logic-embedded.wasm").get().asFile
    inputs.file(coreWasm)
    inputs.file(frameworkWit)
    outputs.file(embeddedWasm)
    doFirst {
        require(frameworkWit.isFile) {
            "Missing ${frameworkWit.absolutePath} (game-core WIT from framework root)"
        }
        require(coreWasm.isFile) { "Missing TeaVM output ${coreWasm.absolutePath}" }
    }
    commandLine(
        "wasm-tools",
        "component",
        "embed",
        "--world",
        "game-core",
        frameworkWit.absolutePath,
        coreWasm.absolutePath,
        "-o",
        embeddedWasm.absolutePath,
    )
}

tasks.register<Exec>("exportLogicComponent") {
    group = "build"
    description = "Package TeaVM core Wasm as an upload-ready WebAssembly Component (logic.wasm)."
    dependsOn("embedLogicComponent", "ensureWasiReactorAdapter")
    val embeddedWasm = layout.buildDirectory.file("out/logic-embedded.wasm").get().asFile
    val outWasm = layout.buildDirectory.file("out/logic.wasm").get().asFile
    val wasiAdapter = wasiReactorAdapter.asFile
    val teavmAdapter = teavmAsciiAdapter.asFile
    inputs.file(embeddedWasm)
    inputs.file(wasiAdapter)
    inputs.file(teavmAdapter)
    outputs.file(outWasm)
    doFirst {
        require(embeddedWasm.isFile) {
            "Missing embedded Wasm ${embeddedWasm.absolutePath}; run embedLogicComponent"
        }
        require(wasiAdapter.isFile) {
            "Missing WASI adapter ${wasiAdapter.absolutePath}; run ensureWasiReactorAdapter"
        }
        require(teavmAdapter.isFile) {
            "Missing TeaVM adapter ${teavmAdapter.absolutePath}; run wasm-tools parse wasm/teavm_ascii.wat -o wasm/teavm_ascii.wasm"
        }
    }
    commandLine(
        "wasm-tools",
        "component",
        "new",
        embeddedWasm.absolutePath,
        "--adapt",
        "wasi_snapshot_preview1=${wasiAdapter.absolutePath}",
        "--adapt",
        "teavm=${teavmAdapter.absolutePath}",
        "-o",
        outWasm.absolutePath,
    )
}

/** @deprecated Use exportLogicComponent; kept for scripts that still call exportLogicWasm. */
tasks.register("exportLogicWasm") {
    group = "build"
    dependsOn("exportLogicComponent")
}
