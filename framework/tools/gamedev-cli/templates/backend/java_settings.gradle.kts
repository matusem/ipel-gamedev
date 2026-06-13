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

rootProject.name = "__ROOT_NAME__"

includeBuild("__SDK_GAME_PATH__") {
    dependencySubstitution {
        substitute(module("sk.upjs.gdd:game")).using(project(":"))
    }
}

include("component")
