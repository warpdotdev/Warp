use warp_util::path::LineAndColumnArg;

use super::{DesktopExecError, EditorMetadata};
use std::path::PathBuf;

#[cfg(test)]
fn with_files(tag: &str, contents: &str, cb: impl FnOnce(PathBuf, PathBuf) -> anyhow::Result<()>) {
    use crate::test_util::{Stub, VirtualFS};

    VirtualFS::test(tag, |dirs, mut sandbox| {
        sandbox.with_files(vec![
            Stub::FileWithContent("bar.desktop", contents),
            Stub::EmptyFile("foo.txt"),
        ]);

        let desktop_file_path = dirs.tests().join("bar.desktop");
        let content_file_path = dirs.tests().join("foo.txt");

        match cb(desktop_file_path, content_file_path) {
            Ok(_) => {}
            Err(err) => panic!("{err:?}"),
        };
    })
}

#[test]
fn test_missing_exec_command_errors() {
    with_files(
        "test_missing_exec_command_errors",
        "",
        |desktop, _content| {
            let result = EditorMetadata::try_new(desktop);

            assert!(matches!(result, Err(DesktopExecError::NoExec)));
            Ok(())
        },
    )
}

#[test]
fn test_exec_ending_on_percent_fails() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=echo "hello world" %
    "#;
    with_files(
        "test_exec_ending_on_percent_fails",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let result = metadata.build_default_command(&content);
            assert!(matches!(result, Err(DesktopExecError::MalformedFieldCode)));
            Ok(())
        },
    )
}

#[test]
fn test_basic_exec_no_field_codes() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=echo "hello world"
    "#;
    with_files(
        "test_basic_exec_no_field_codes",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let result = metadata.build_default_command(&content);
            assert!(result.is_ok());
            let cmd = result.unwrap();
            assert_eq!(cmd.get_program(), "echo");
            assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["hello world"]);
            Ok(())
        },
    )
}

#[test]
fn test_file_path_substitution() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=cat %f
    "#;
    with_files("test_file_path_substitution", data, |desktop, content| {
        let metadata = EditorMetadata::try_new(desktop)?;
        let file_name = content.display().to_string();
        let result = metadata.build_default_command(&content);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().get_args().collect::<Vec<_>>(),
            [file_name.as_str()]
        );
        Ok(())
    });

    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=cat %F
    "#;
    with_files("test_file_path_substitution", data, |desktop, content| {
        let metadata = EditorMetadata::try_new(desktop)?;
        let file_name = content.display().to_string();
        let result = metadata.build_default_command(&content);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().get_args().collect::<Vec<_>>(),
            [file_name.as_str()]
        );
        Ok(())
    });
}

#[test]
fn test_file_url_substitution() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=open %u
    "#;
    with_files("test_file_url_substitution", data, |desktop, content| {
        let metadata = EditorMetadata::try_new(desktop)?;
        let file_name = content.display().to_string();
        let expected_file_uri = format!("file://{file_name}");
        let result = metadata.build_default_command(&content);

        assert!(result.is_ok());

        assert_eq!(
            result.unwrap().get_args().collect::<Vec<_>>(),
            [expected_file_uri.as_str()]
        );
        Ok(())
    });

    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=open %U
    "#;
    with_files("test_file_url_substitution", data, |desktop, content| {
        let metadata = EditorMetadata::try_new(desktop)?;
        let file_name = content.display().to_string();
        let expected_file_uri = format!("file://{file_name}");
        let result = metadata.build_default_command(&content);

        assert!(result.is_ok());

        assert_eq!(
            result.unwrap().get_args().collect::<Vec<_>>(),
            [expected_file_uri.as_str()]
        );
        Ok(())
    });
}

#[test]
fn test_remaining_substitutions() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=echo %c && echo %i && echo %k && echo %%
    Name=Warp Test Application
    Icon=/foo/bar/icon.png
    "#;
    with_files("test_remaining_substitutions", data, |desktop, content| {
        let desktop_file_path = desktop.display().to_string();
        let metadata = EditorMetadata::try_new(desktop)?;
        let result = metadata.build_default_command(&content);

        assert!(result.is_ok());

        // 基于 argv 构造命令时，每个 token 都是独立参数。
        // %c → "Warp Test Application"（单个参数，保留空格）
        // %i → "--icon" 和 "/foo/bar/icon.png"（两个独立参数）
        // %k → desktop 文件路径
        // %% → "%"
        let cmd = result.unwrap();
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args[0], "Warp Test Application");
        assert_eq!(args[1], "&&");
        assert_eq!(args[2], "echo");
        assert_eq!(args[3], "--icon");
        assert_eq!(args[4], "/foo/bar/icon.png");
        assert_eq!(args[5], "&&");
        assert_eq!(args[6], "echo");
        assert_eq!(args[7], desktop_file_path.as_str());
        assert_eq!(args[8], "&&");
        assert_eq!(args[9], "echo");
        assert_eq!(args[10], "%");
        Ok(())
    });
}

#[test]
fn test_jetbrains_command_no_line_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/phpstorm %f
    "#;

    with_files(
        "test_jetbrains_command_no_line_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result = metadata.build_jetbrains_command(&content, None);

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                [file_path.as_str()]
            );
            Ok(())
        },
    );
}

#[test]
fn test_jetbrains_command_line_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/phpstorm %f
    "#;

    with_files(
        "test_jetbrains_command_line_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result = metadata.build_jetbrains_command(
                &content,
                Some(LineAndColumnArg {
                    line_num: 42,
                    column_num: None,
                }),
            );

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                ["--line", "42", file_path.as_str()]
            );
            Ok(())
        },
    );
}

#[test]
fn test_jetbrains_command_line_and_col_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/phpstorm %f
    "#;
    with_files(
        "test_jetbrains_command_line_and_col_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result = metadata.build_jetbrains_command(
                &content,
                Some(LineAndColumnArg {
                    line_num: 42,
                    column_num: Some(25),
                }),
            );

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                ["--line", "42", "--column", "25", file_path.as_str()]
            );
            Ok(())
        },
    );
}

#[test]
fn test_sublime_command_no_line_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/subl %f
    "#;
    with_files(
        "test_sublime_command_no_line_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result: Result<command::blocking::Command, DesktopExecError> =
                metadata.build_sublime_command(&content, None);

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                [file_path.as_str()]
            );
            Ok(())
        },
    );
}

#[test]
fn test_sublime_command_line_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/subl %f
    "#;
    with_files(
        "test_sublime_command_line_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result = metadata.build_sublime_command(
                &content,
                Some(LineAndColumnArg {
                    line_num: 42,
                    column_num: None,
                }),
            );

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                [format!("{file_path}:42").as_str()]
            );
            Ok(())
        },
    );
}

#[test]
fn test_sublime_command_line_and_col_numbers() {
    let data = r#"
    [Desktop Entry]
    Version=1.0
    Type=Application
    Exec=/snap/bin/subl %f
    "#;
    with_files(
        "test_sublime_command_line_numbers",
        data,
        |desktop, content| {
            let metadata = EditorMetadata::try_new(desktop)?;
            let file_path = content.display().to_string();
            let result = metadata.build_sublime_command(
                &content,
                Some(LineAndColumnArg {
                    line_num: 42,
                    column_num: Some(25),
                }),
            );

            assert!(result.is_ok());

            assert_eq!(
                result.unwrap().get_args().collect::<Vec<_>>(),
                [format!("{file_path}:42:25").as_str()]
            );
            Ok(())
        },
    );
}
