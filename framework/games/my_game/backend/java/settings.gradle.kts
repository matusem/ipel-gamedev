pluginManagement {
    repositories {
        mavenCentral()
        gradlePluginPortal()
    }
    plugins {
        id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention")
}

@Suppress("UnstableApiUsage")
dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

rootProject.name = "my_game-java"

includeBuild("../../../../sdk/java/game") {
    dependencySubstitution {
        substitute(module("dev.ipel.gamedev:game")).using(project(":"))
    }
}

include("component")
