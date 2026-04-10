/**
 * Guest Wasm is built with **Maven** + [com.fermyon teavm-wasi](https://github.com/fermyon/teavm-wasi)
 * (`pom.xml`), not the upstream `org.teavm` Gradle plugin (browser Wasm GC).
 *
 * Uses the **Maven Wrapper** (`mvnw` / `mvnw.cmd`) in this directory (no global `mvn`).
 * Requires a composite `:game:jar` for `dev.ipel.gamedev:game`.
 *
 * TeaVM 0.2.x cannot parse JDK 21 bootstrap classes (major 65); `mavenPackage` runs Maven with **JDK 17** via toolchains.
 */

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

tasks.register<Copy>("exportLogicWasm") {
    group = "build"
    description = "Stage TeaVM output as build/out/logic.wasm (for gamedev-cli / packaging)."
    dependsOn("mavenPackage")
    from(layout.projectDirectory.dir("target/generated/wasm/teavm-wasm")) {
        include("*.wasm")
        rename { _ -> "logic.wasm" }
    }
    into(layout.buildDirectory.dir("out"))
    duplicatesStrategy = DuplicatesStrategy.FAIL
}
