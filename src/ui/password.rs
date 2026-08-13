//! The password prompt — P2 §5, and CORE §4's seventh popup.
//!
//! "a modal popup: masked field, `Enter` confirms, `Esc` cancels, nothing echoed
//! anywhere, value carried as `Secret` end to end."
//!
//! CORE §9: "Passwords are never stored or remembered — typed per use, wiped after."
//! Nothing in this file writes the value anywhere but into a `Secret` — with one copy it
//! cannot avoid. An egui text field types into a `String`, so `Indium::password_input` and
//! `password_confirm` hold the characters themselves until submit or cancel clears them, and
//! `String::clear` sets the length to zero and leaves the bytes in the allocation where
//! `Secret`'s `write_volatile` would have overwritten them. Written down rather than papered
//! over, and recorded in CORE §3's `secret` row as the second copy the type cannot follow.

use eframe::egui;

use super::{Indium, PendingAction, Popup, Status};
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

    // egui's default backdrop is `from_black_alpha(100)`, and pure black over an aubergine
    // window pulls the whole thing grey at the exact moment the user is being asked for a
    // secret. `SCRIM` is `VOID` at 78%, so the window stays in its own family and the dimming
    // is nearly twice as deep — which is what "everything else is unavailable" should look
    // like. The `Frame` is left alone deliberately: `Modal` defaults to `Frame::popup`, which
    // reads `window_fill`, `window_stroke` and `menu_corner_radius` from `install_visuals`,
    // so the modal already takes the new POPUP ground without an override here. P7 §3.
    egui::Modal::new(egui::Id::new("password-prompt"))
        .backdrop_color(theme::SCRIM)
        .show(ctx, |ui| {
            ui.set_width(400.0);
            // A `Modal` draws no title bar, so this label *is* the title, and it takes the
            // Heading style every other popup's title is drawn in rather than a hand-typed 17.0
            // that would drift the moment the scale moved. Not `theme::section_bare`: a section
            // heading is 14.0 with a gap above it, which would make the one popup that cannot
            // draw its own title bar wear the smallest title in the program.
            ui.label(egui::RichText::new("Password").heading().color(theme::TEXT));
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(match &app.pending {
                    Some(PendingAction::List(_)) => {
                        "This archive's file names are encrypted. A password is needed to list it."
                    }
                    Some(PendingAction::Crc { .. })
                    | Some(PendingAction::OpenWith { .. })
                    | Some(PendingAction::Preview { .. }) => {
                        "This entry is encrypted. A password is needed to read it."
                    }
                    Some(PendingAction::CopyOut) => {
                        "This selection is encrypted. A password is needed to copy it out."
                    }
                    // Its own arm because the `_` below would otherwise have taken it and
                    // said "extract", which is what a person pressing *Bring from archive*
                    // has not asked for. The compiler catches the dispatch at the foot of
                    // this file and cannot catch this: a catch-all is never a missing arm.
                    Some(PendingAction::Draft) => {
                        "This selection is encrypted. A password is needed to bring it into \
                     the draft."
                    }
                    Some(PendingAction::Apply) => {
                        "Choose the password for this archive. INDIUM never stores it, so \
                     there is no way to recover it if you forget it."
                    }
                    _ => "This selection is encrypted. A password is needed to extract it.",
                })
                .size(13.0)
                .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(10.0);

            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.password_input)
                    .password(true)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            // Once, on opening. Every frame is what stopped the confirm box below from
            // ever holding focus — see `Indium::wants_initial_focus`.
            if app.wants_initial_focus(&Popup::Password) {
                resp.request_focus();
            }

            // A fresh encrypted archive is asked twice. There is nothing to check a typo
            // against — no existing archive to try the password on — and a typo would build
            // something nobody, including its author, can ever open.
            let confirming =
                app.pending == Some(PendingAction::Apply) && app.tasks.creates_encrypted();
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
                            .size(13.0)
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
                    .size(13.0)
                    .color(theme::WARNING),
                );
            }

            ui.add_space(10.0);
            let ready = !confirming
                || (!app.password_input.is_empty() && app.password_confirm == app.password_input);
            ui.horizontal(|ui| {
                let label = if confirming { "Set" } else { "Unlock" };
                if theme::button(ui, egui::RichText::new(label), ready).clicked() {
                    submit = true;
                }
                if theme::button(ui, egui::RichText::new("Cancel"), true).clicked() {
                    cancel = true;
                }
                ui.label(
                    egui::RichText::new("Never stored — used once, then wiped.")
                        .size(12.0)
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
        app.status = "Cancelled. Nothing was written.".to_string().into();
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
            app.status =
                Status::bad("Wrong password three times. Cancelled — nothing was written.");
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
            // Both of these spawn a worker that takes its own clone of the passphrase
            // before returning, so clearing it here still leaves the extraction able to
            // read what it was unlocked for.
            app.passphrase = Some(secret);
            let rows = app.rows();
            app.copy_out(ctx, &rows);
            app.passphrase = None;
        }
        Some(PendingAction::Draft) => {
            // Carries nothing, and needs to carry nothing: the selection it acts on is
            // still on the window, because a password prompt does not change what is
            // selected. So the resume is the first press, repeated with the secret in hand.
            app.passphrase = Some(secret);
            app.bring_from_archive(ctx);
            app.passphrase = None;
        }
        Some(PendingAction::OpenWith { entry }) => {
            app.passphrase = Some(secret);
            app.open_with(ctx, &entry);
            app.passphrase = None;
        }
        Some(PendingAction::Preview { entry }) => {
            app.passphrase = Some(secret);
            app.request_preview(ctx, &entry);
            // The worker holds its own clone, and Preview caches the bytes rather than
            // the password.
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
