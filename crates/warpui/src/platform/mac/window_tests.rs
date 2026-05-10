use super::super::make_nsstring;
use super::super::AutoreleasePoolGuard;
use super::to_string;

unsafe fn nsstring(s: &str) -> cocoa::base::id {
    make_nsstring(s)
}

#[test]
fn to_string_ascii() {
    let _pool = AutoreleasePoolGuard::new();
    unsafe {
        assert_eq!(to_string(nsstring("hello world")), "hello world");
    }
}

#[test]
fn to_string_chinese() {
    let _pool = AutoreleasePoolGuard::new();
    unsafe {
        assert_eq!(to_string(nsstring("中文")), "中文");
    }
}

#[test]
fn to_string_japanese() {
    let _pool = AutoreleasePoolGuard::new();
    unsafe {
        assert_eq!(to_string(nsstring("日本語")), "日本語");
    }
}

#[test]
fn to_string_emoji() {
    let _pool = AutoreleasePoolGuard::new();
    unsafe {
        assert_eq!(to_string(nsstring("🎉")), "🎉");
    }
}

#[test]
fn to_string_mixed_cjk() {
    let _pool = AutoreleasePoolGuard::new();
    unsafe {
        assert_eq!(
            to_string(nsstring("hello 中文 world 日本語 test")),
            "hello 中文 world 日本語 test"
        );
    }
}
