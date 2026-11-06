mod support;

use support::{Fixture, command, has_note, ids, roots};
use z_engine_protocol::CheckKind;

const GRADLEW: &str = if cfg!(windows) {
    r".\gradlew.bat"
} else {
    "./gradlew"
};
const MVNW: &str = if cfg!(windows) {
    r".\mvnw.cmd"
} else {
    "./mvnw"
};

#[test]
fn gradle_builds_use_the_wrapper_and_cover_subprojects() {
    let fixture = Fixture::new(&[
        (
            "settings.gradle.kts",
            "rootProject.name = \"shop\"\ninclude(\"api\", \"web\")\n",
        ),
        ("build.gradle.kts", "plugins { java }\n"),
        ("gradlew", "#!/bin/sh\n"),
        ("gradlew.bat", "@echo off\n"),
        ("api/build.gradle.kts", "plugins { java }\n"),
        ("web/build.gradle", "apply plugin: 'java'\n"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "gradle")]);
    assert_eq!(profile.roots[0].manifest, "settings.gradle.kts");
    assert_eq!(ids(&profile), ["gradle:test", "gradle:build"]);
    assert_eq!(command(&profile, "gradle:test"), format!("{GRADLEW} test"));
    assert_eq!(
        command(&profile, "gradle:build"),
        format!("{GRADLEW} build")
    );

    let standalone = Fixture::new(&[("tools/lint/build.gradle", "apply plugin: 'java'\n")]);
    let profile = standalone.discover();
    assert_eq!(roots(&profile), [("tools/lint", "gradle")]);
    assert_eq!(command(&profile, "tools/lint/gradle:test"), "gradle test");
}

#[test]
fn maven_modules_are_covered_by_the_parent_build() {
    let pom = "<project><modelVersion>4.0.0</modelVersion><artifactId>shop</artifactId></project>";
    let fixture = Fixture::new(&[
        ("pom.xml", pom),
        ("mvnw", "#!/bin/sh\n"),
        ("mvnw.cmd", "@echo off\n"),
        ("core/pom.xml", pom),
        ("broken/nested/pom.xml", "not xml"),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "maven")]);
    assert_eq!(ids(&profile), ["maven:test", "maven:package"]);
    assert_eq!(command(&profile, "maven:test"), format!("{MVNW} -q test"));
    assert_eq!(
        command(&profile, "maven:package"),
        format!("{MVNW} -q -DskipTests package")
    );
    assert!(has_note(
        &profile,
        "broken/nested/pom.xml: no <project> element"
    ));

    let plain = Fixture::new(&[("svc/pom.xml", pom)]).discover();
    assert_eq!(command(&plain, "svc/maven:test"), "mvn -q test");
}

#[test]
fn dotnet_solutions_cover_their_projects() {
    let sln = "\u{feff}\nMicrosoft Visual Studio Solution File, Format Version 12.00\n";
    let fixture = Fixture::new(&[
        ("Shop.sln", sln),
        (
            "src/Shop/Shop.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk"></Project>"#,
        ),
        (
            "tests/Shop.Tests/Shop.Tests.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk"><ItemGroup><PackageReference Include="Microsoft.NET.Test.Sdk" /></ItemGroup></Project>"#,
        ),
    ]);
    let profile = fixture.discover();
    assert_eq!(roots(&profile), [(".", "dotnet")]);
    assert_eq!(profile.roots[0].manifest, "Shop.sln");
    assert_eq!(ids(&profile), ["dotnet:test", "dotnet:build"]);
    assert_eq!(command(&profile, "dotnet:test"), "dotnet test");
}

#[test]
fn dotnet_projects_without_a_solution() {
    let fixture = Fixture::new(&[
        (
            "lib/Lib.csproj",
            r#"<Project Sdk="Microsoft.NET.Sdk"></Project>"#,
        ),
        (
            "Lib.Tests/Lib.Tests.fsproj",
            "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><IsTestProject>true</IsTestProject></PropertyGroup></Project>",
        ),
        (
            "both/My App.sln",
            "Microsoft Visual Studio Solution File, Format Version 12.00\n",
        ),
        ("both/App.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\" />"),
    ]);
    let profile = fixture.discover();
    assert_eq!(
        roots(&profile),
        [
            ("Lib.Tests", "dotnet"),
            ("both", "dotnet"),
            ("lib", "dotnet")
        ]
    );
    assert_eq!(
        ids(&profile)[..2],
        ["Lib.Tests/dotnet:test", "Lib.Tests/dotnet:build"]
    );
    assert_eq!(command(&profile, "lib/dotnet:build"), "dotnet build");
    assert!(!ids(&profile).contains(&"lib/dotnet:test"));
    assert_eq!(
        command(&profile, "both/dotnet:test"),
        "dotnet test \"My App.sln\""
    );
    let build = profile
        .checks
        .iter()
        .find(|c| c.id == "both/dotnet:build")
        .unwrap();
    assert_eq!(build.kind, CheckKind::Build);
}

#[test]
fn invalid_dotnet_files_are_notes() {
    let fixture = Fixture::new(&[
        ("a/App.csproj", "<Wrong/>"),
        ("b/App.sln", "not a solution"),
    ]);
    let profile = fixture.discover();
    assert!(profile.roots.is_empty());
    assert!(has_note(
        &profile,
        "a/App.csproj: not a solution or MSBuild project file"
    ));
    assert!(has_note(
        &profile,
        "b/App.sln: not a solution or MSBuild project file"
    ));
}
