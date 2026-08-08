//! The password prompt — P2 §5, and CORE §4's seventh popup.
//!
//! "a modal popup: masked field, `Enter` confirms, `Esc` cancels, nothing echoed
//! anywhere, value carried as `Secret` end to end."
//!
//! CORE §9: "Passwords are never stored or remembered — typed per use, wiped after."
//! Nothing in this file writes the value anywhere but into a `Secret`.

use eframe::egui;

use super::{Indium, PendingAction, Popup};
use crate::arch;
use crate::secret::Secret;
use crate::theme;

/// P2 §5: "A wrong password re-prompts, three attempts, then cancels."
const MAX_ATTEMPTS: u8 = 3;

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Password) {
        return;
    }

    let mut submit = false;
    let mut cancel = false;

    egui::Modal::new(egui::Id::new("password-prompt")).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.label(
            egui::RichText::new("Password")
                .size(15.0)
                .color(theme::TEXT),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(match &app.pending {
                Some(PendingAction::List(_)) => {
                    "This archive's file names are encrypted. A password is needed to list it."
                }
                Some(PendingAction::Crc { .. }) | Some(PendingAction::OpenWith { .. }) => {
                    "This entry is encrypted. A password is needed to read it."
                }
                Some(PendingAction::CopyOut) => {
                    "This selection is encrypted. A password is needed to copy it out."
                }
                Some(PendingAction::Apply) => {
                    "Choose the password for this archive. INDIUM never stores it, so \
                     there is no way to recover it if you forget it."
                }
                _ => "This selection is encrypted. A password is needed to extract it.",
            })
            .size(11.0)
            .color(theme::TEXT_SECONDARY),
        );
        ui.add_space(10.0);

        let resp = ui.add(
            egui::TextEdit::singleline(&mut app.password_input)
                .password(true)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY),
        );
        resp.request_focus();

        // A fresh encrypted archive is asked twice. There is nothing to check a typo
        // against — no existing archive to try the password on — and a typo would build
        // something nobody, including its author, can ever open.
        let confirming = app.pending == Some(PendingAction::Apply) && app.tasks.creates_encrypted();
        if confirming {
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.password_confirm)
                    .password(true)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("confirm")
                    .desired_width(f32::INFINITY),
            );
            if !app.password_confirm.is_empty() && app.password_confirm != app.password_input {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("The two passwords are not the same.")
                        .size(11.0)
                        .color(theme::WARNING),
                );
            }
        }

        if app.password_attempts > 0 {
            let left = MAX_ATTEMPTS.saturating_sub(app.password_attempts);
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Wrong password. {left} {} left.",
                    if left == 1 { "attempt" } else { "attempts" }
                ))
                .size(11.0)
                .color(theme::WARNING),
            );
        }

        ui.add_space(10.0);
        let ready = !confirming
            || (!app.password_input.is_empty() && app.password_confirm == app.password_input);
        ui.horizontal(|ui| {
            let label = if confirming { "Set" } else { "Unlock" };
            if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                submit = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
            ui.label(
                egui::RichText::new("Never stored — used once, then wiped.")
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
        });

        if ready && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
    });

    if cancel {
        dismiss(app);
        app.status = "Cancelled. Nothing was written.".to_string();
        return;
    }
    if submit && !app.password_input.is_empty() {
        attempt(app, ctx);
    }
}

fn attempt(app: &mut Indium, ctx: &egui::Context) {
    let secret = Secret::from_text(&app.password_input);
    // Both plain-text fields are cleared the moment their contents are in a Secret.
    app.password_input.clear();
    app.password_confirm.clear();

    let Some(archive) = app.archive_path.clone() else {
        dismiss(app);
        return;
    };

    let pending = app.pending.clone();

    // For encrypted *headers* there is nothing to verify against without opening;
    // the reopen itself is the test. For encrypted *entries*, P2 §5 requires the
    // throwaway-reader check so a wrong password writes nothing.
    // A password being *chosen* for an archive that does not exist yet cannot be
    // verified against anything — there is nothing to try it on. That is exactly why it
    // was typed twice, and the confirmation is the check.
    let choosing = pending == Some(PendingAction::Apply) && app.tasks.creates_encrypted();

    let accepted = match &pending {
        _ if choosing => true,
        Some(PendingAction::List(_)) => arch::list_all(&archive, Some(&secret)).is_ok(),
        _ => arch::verify_passphrase(&archive, &secret).unwrap_or(false),
    };

    if !accepted {
        app.password_attempts += 1;
        if app.password_attempts >= MAX_ATTEMPTS {
            dismiss(app);
            app.status = "Wrong password three times. Cancelled — nothing was written.".to_string();
        }
        return;
    }

    app.password_attempts = 0;
    app.popup = None;
    app.pending = None;

    match pending {
        Some(PendingAction::List(path)) => {
            app.open_archive(ctx, path, Some(secret));
        }
        Some(PendingAction::Extract { dest }) => {
            app.passphrase = Some(secret);
            app.begin_extract(ctx, dest);
        }
        Some(PendingAction::Crc { entry }) => {
            app.passphrase = Some(secret);
            app.compute_crc(&entry);
            // The password's job is done the moment the checksum is in hand.
            app.passphrase = None;
        }
        Some(PendingAction::CopyOut) => {
            app.passphrase = Some(secret);
            let rows = app.rows();
            app.copy_out(&rows);
            app.passphrase = None;
        }
        Some(PendingAction::OpenWith { entry }) => {
            app.passphrase = Some(secret);
            app.open_with(&entry);
            app.passphrase = None;
        }
        Some(PendingAction::Apply) => {
            // Held for the whole rebuild — it may be needed to read the source, to
            // encrypt the replacement, or both — and dropped when the worker reports.
            app.passphrase = Some(secret);
            if let Some(recipe) = app.current_recipe() {
                app.begin_apply(ctx, recipe);
            }
        }
        None => {}
    }
}

fn dismiss(app: &mut Indium) {
    app.password_input.clear();
    app.password_confirm.clear();
    app.password_attempts = 0;
    app.pending = None;
    app.popup = None;
    app.passphrase = None;
}
