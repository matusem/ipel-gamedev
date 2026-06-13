//! Gradle 8+ requires JVM 17+ to run. Many Windows installs default to Java 8 on PATH — fail fast with a clear hint.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const MIN_JAVA_MAJOR_FOR_GRADLE: u32 = 17;

/// `java` used by Gradle: `JAVA_HOME/bin/java` when set and present, otherwise `java` on PATH.
pub(crate) fn resolved_java_executable() -> PathBuf {
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let exe_name = if cfg!(windows) { "java.exe" } else { "java" };
        let p = Path::new(&home).join("bin").join(exe_name);
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("java")
}

/// Parses `java -version` stderr, e.g. `openjdk version "21.0.5"` or `java version "1.8.0_391"`.
fn java_major_from_version_output(stderr: &str) -> Option<u32> {
    let needle = "version \"";
    let idx = stderr.find(needle)?;
    let start = idx + needle.len();
    let rest = &stderr[start..];
    let end = rest.find('"')?;
    let ver = &rest[..end];
    let mut parts = ver.split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

pub(crate) fn ensure_java_for_gradle() -> Result<()> {
    let java = resolved_java_executable();
    let output = Command::new(&java)
        .arg("-version")
        .output()
        .with_context(|| {
            format!(
                "failed to run `{} -version`; install a JDK and/or fix JAVA_HOME",
                java.display()
            )
        })?;

    let text = String::from_utf8_lossy(&output.stderr);
    let major = java_major_from_version_output(&text).or_else(|| {
        let out = String::from_utf8_lossy(&output.stdout);
        java_major_from_version_output(&out)
    });

    let Some(major) = major else {
        bail!(
            "could not parse Java version from `{} -version` output:\n{text}\n\
             Set JAVA_HOME to JDK {} or newer.",
            java.display(),
            MIN_JAVA_MAJOR_FOR_GRADLE
        );
    };

    if major < MIN_JAVA_MAJOR_FOR_GRADLE {
        bail!(
            "Gradle requires JVM {} or newer to run; `{}` reports Java {}.\n\
             Install JDK {}+ (e.g. Eclipse Temurin 21) and set JAVA_HOME to that JDK, \
             or put its `bin` directory before older Java on PATH.\n\
             See framework/sdk/java/README.md.",
            MIN_JAVA_MAJOR_FOR_GRADLE,
            java.display(),
            major,
            MIN_JAVA_MAJOR_FOR_GRADLE
        );
    }

    Ok(())
}
