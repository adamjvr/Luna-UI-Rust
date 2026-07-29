// SPDX-License-Identifier: MPL-2.0

//! Private native-dialog response normalization.
//!
//! Zenity extra buttons are not ordinary affirmative buttons. Depending on the release and dialog
//! mode, selecting an extra button can use the same non-zero process status as Cancel while writing
//! the selected label to an output stream. GTK diagnostics may also be written to standard error.
//! The product-neutral dialog contract must therefore identify an exact extra-button response before
//! interpreting the process status.

/// Returns whether Zenity reported the selected extra-button label.
///
/// An exact, ASCII-whitespace-trimmed response line on either output stream is accepted. Diagnostic
/// lines that merely contain the label as a substring are not responses. Byte matching avoids making
/// native toolkit diagnostics part of Luna's UTF-8 contract.
#[must_use]
pub(super) fn zenity_extra_button_selected(
    stdout: &[u8],
    stderr: &[u8],
    extra_label: &str,
) -> bool {
    output_contains_response_line(stdout, extra_label.as_bytes())
        || output_contains_response_line(stderr, extra_label.as_bytes())
}

fn output_contains_response_line(output: &[u8], expected: &[u8]) -> bool {
    output
        .split(|byte| matches!(*byte, b'\n' | b'\r'))
        .map(trim_ascii_whitespace)
        .any(|line| line == expected)
}

fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[1..];
    }
    while value.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::zenity_extra_button_selected;

    #[test]
    fn empty_output_is_not_an_extra_button() {
        assert!(!zenity_extra_button_selected(b"", b"", "Discard"));
    }

    #[test]
    fn extra_button_on_stdout_is_recognized() {
        assert!(zenity_extra_button_selected(b"Discard\n", b"", "Discard"));
    }

    #[test]
    fn extra_button_on_stderr_is_recognized() {
        assert!(zenity_extra_button_selected(
            b"",
            b"Gtk-WARNING: portal detail\nDiscard\n",
            "Discard"
        ));
    }

    #[test]
    fn response_must_be_a_complete_trimmed_line() {
        assert!(!zenity_extra_button_selected(
            b"",
            b"Gtk-WARNING: Discard was mentioned in a diagnostic\n",
            "Discard"
        ));
        assert!(zenity_extra_button_selected(
            b"\r\n  Discard  \r\n",
            b"",
            "Discard"
        ));
    }
}
