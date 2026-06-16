plugins {
    `java-library`
    `maven-publish`
}

group = "sk.upjs.gdd"
version = "0.1.0"

java {
    toolchain {
        languageVersion.set(JavaLanguageVersion.of(21))
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.release.set(17)
}

repositories {
    mavenCentral()
}

dependencies {
    api("com.fasterxml.jackson.core:jackson-databind:2.14.3")
    api("org.msgpack:jackson-dataformat-msgpack:0.9.8")

    testImplementation(platform("org.junit:junit-bom:5.11.4"))
    testImplementation("org.junit.jupiter:junit-jupiter")
}

tasks.test {
    useJUnitPlatform()
}

tasks.register<JavaExec>("exportJsonSchema") {
    group = "gamedev"
    description = "Emit JSON Schema IR for client codegen"
    classpath = sourceSets["main"].runtimeClasspath
    mainClass.set("sk.upjs.gdd.game.tooling.ExportJsonSchema")
    args(layout.buildDirectory.dir("schema").get().asFile.absolutePath)
}

// Maven Central publication — see docs/sdk-publishing.md
// Apply com.vanniktech.maven.publish plugin in CI when releasing.
publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])
            groupId = project.group.toString()
            artifactId = "game"
            version = project.version.toString()
            pom {
                name.set("UPJŠ GDD Platform Java SDK")
                description.set("Portable game library for Java/TeaVM backends on UPJŠ GDD Platform")
                url.set("https://github.com/matusem/ipel-gamedev")
                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }
                }
                scm {
                    connection.set("scm:git:git://github.com/matusem/ipel-gamedev.git")
                    developerConnection.set("scm:git:ssh://github.com:matusem/ipel-gamedev.git")
                    url.set("https://github.com/matusem/ipel-gamedev")
                }
            }
        }
    }
}
