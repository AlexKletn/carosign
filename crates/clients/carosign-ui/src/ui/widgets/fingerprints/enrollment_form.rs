use std::cell::{Cell, RefCell};

use carosign_l10n::Localizer;
use carosign_protocol::carosign::{
    client::v1::OperationEvent,
    common::v1::{
        AcceptedSamples, CaptureAction, CaptureFeedback, CapturePrompt, CaptureTarget,
        capture_feedback::Kind,
    },
};
use obscur_kit::view::view::View;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use super::{enrollment_capture::EnrollmentCapture, enrollment_print, enrollment_ring};

const CAPTURES: u32 = 20;

pub(super) struct EnrollmentForm {
    screen: OkuiScreen,
    draft: RefCell<Option<(String, EnrollmentCapture)>>,
    fit: Cell<(f32, f32)>,
    live: Cell<bool>,
    running: Cell<bool>,
    cancelling: Cell<bool>,
    completion: Cell<Option<&'static str>>,
    finishing: Cell<bool>,
}

impl EnrollmentForm {
    pub(super) fn new() -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("value", "");
        data.set("form_style", "");
        data.set("name_label", "");
        data.set("progress_class", "status");
        data.set("event_class", "value enrollment-event");
        data.set("field_class", "enrollment-field-hidden");
        data.set("action_disabled", false);
        data.set("action_class", "enrollment-primary");
        data.set("cancel_disabled", false);
        data.set("name_disabled", false);
        data.set("discard_class", "enrollment-discard-hidden");
        Ok(Self {
            screen: OkuiScreen::compile_with(include_str!("enrollment_form.okui"), data)?,
            draft: RefCell::default(),
            fit: Cell::new((1.0, 0.0)),
            live: Cell::default(),
            running: Cell::default(),
            cancelling: Cell::default(),
            completion: Cell::default(),
            finishing: Cell::default(),
        })
    }

    pub(super) fn is_open(&self) -> bool {
        self.draft.borrow().is_some()
    }

    pub(super) fn open(&self, label: String) {
        *self.draft.borrow_mut() = Some((label, EnrollmentCapture::new(CAPTURES)));
    }

    pub(super) fn open_live(&self) {
        self.live.set(true);
        self.running.set(true);
        *self.draft.borrow_mut() = Some((String::new(), EnrollmentCapture::new(0)));
    }

    pub(super) fn captured(&self) {
        self.running.set(false);
        self.cancelling.set(false);
        if let Some((_, capture)) = self.draft.borrow_mut().as_mut() {
            capture.percent = 100.0;
        }
    }

    pub(super) fn captured_name(&self) -> Option<String> {
        let draft = self.draft.borrow();
        let label = draft.as_ref()?.0.trim();
        (!label.is_empty() && label.len() <= carosign_protocol::limits::LABEL_MAX)
            .then(|| label.to_owned())
    }

    pub(super) fn finish_live(&self, message: &'static str, pending: bool) {
        self.completion.set(Some(message));
        self.finishing.set(pending);
    }

    pub(super) fn apply(&self, event: &OperationEvent) {
        if let Some((_, capture)) = self.draft.borrow_mut().as_mut() {
            capture.apply(event);
        }
    }

    pub(super) fn cancelling(&self) {
        self.cancelling.set(true);
    }

    pub(super) fn close(&self) -> bool {
        self.completion.set(None);
        self.finishing.set(false);
        self.live.set(false);
        self.running.set(false);
        self.cancelling.set(false);
        self.draft.borrow_mut().take().is_some()
    }

    pub(super) fn edit(&self, label: &str) -> bool {
        if self.running.get() || self.finishing.get() {
            return false;
        }
        let mut draft = self.draft.borrow_mut();
        let Some((value, _)) = draft.as_mut() else {
            return false;
        };
        *value = label.to_owned();
        true
    }

    pub(super) fn is_ready(&self) -> bool {
        self.draft
            .borrow()
            .as_ref()
            .is_some_and(|(_, capture)| capture.accepted == CAPTURES)
    }

    pub(super) fn scan(&self) -> bool {
        let mut draft = self.draft.borrow_mut();
        let Some((_, capture)) = draft.as_mut() else {
            return false;
        };
        if capture.accepted == CAPTURES {
            return false;
        }
        let accepted = capture.accepted + 1;
        capture.apply(&OperationEvent {
            feedback: Some(CaptureFeedback {
                kind: Some(Kind::AcceptedSamples(AcceptedSamples {
                    accepted,
                    required: CAPTURES,
                })),
            }),
            ..OperationEvent::default()
        });
        if accepted < CAPTURES {
            let action = match accepted % 3 {
                1 => CaptureAction::Move,
                2 => CaptureAction::Center,
                _ => CaptureAction::Place,
            };
            capture.apply(&OperationEvent {
                feedback: Some(CaptureFeedback {
                    kind: Some(Kind::Prompt(CapturePrompt {
                        action: action as i32,
                        target: CaptureTarget::Finger as i32,
                    })),
                }),
                ..OperationEvent::default()
            });
        }
        true
    }

    pub(super) fn finish(&self) -> Option<String> {
        let mut draft = self.draft.borrow_mut();
        let (label, capture) = draft.as_ref()?;
        let label = label.trim();
        if capture.accepted != CAPTURES || label.is_empty() {
            return None;
        }
        let label = label.to_owned();
        *draft = None;
        Some(label)
    }

    pub(super) fn set_fit(&self, scale: f32, offset: f32) -> bool {
        let previous = self.fit.get();
        if (previous.0 - scale).abs() < 0.0001 && (previous.1 - offset).abs() < 0.01 {
            return false;
        }
        self.fit.set((scale, offset));
        true
    }

    pub(super) fn view(&self, localizer: &Localizer, motion: bool) -> View {
        let draft = self.draft.borrow();
        let Some((label, capture)) = draft.as_ref() else {
            return View::empty();
        };
        let live = self.live.get();
        let running = self.running.get();
        let ready = if live {
            !running
        } else {
            capture.accepted == CAPTURES
        };
        let data = self.screen.data();
        data.set("value", label.as_str());
        data.set("name_disabled", self.finishing.get());
        let (scale, offset) = self.fit.get();
        data.set(
            "form_style",
            if scale < 1.0 || offset.abs() >= 0.01 {
                format!("transform: translateY({offset:.3}px) scale({scale:.5})")
            } else {
                String::new()
            },
        );
        data.set("name_label", localizer.text("ui-enroll-name"));
        data.set(
            "field_class",
            if ready {
                "enrollment-field"
            } else {
                "enrollment-field-hidden"
            },
        );
        data.set(
            "action_label",
            localizer.text(if live {
                "ui-enroll-save-name"
            } else if ready {
                "ui-enroll-save"
            } else {
                "ui-enroll-simulate-touch"
            }),
        );
        data.set(
            "action_disabled",
            self.finishing.get()
                || (ready
                    && (label.trim().is_empty()
                        || label.trim().len() > carosign_protocol::limits::LABEL_MAX)),
        );
        data.set(
            "action_class",
            if running {
                "enrollment-discard-hidden"
            } else {
                "enrollment-primary"
            },
        );
        data.set(
            "cancel_disabled",
            self.cancelling.get() || self.finishing.get(),
        );
        data.set(
            "discard_class",
            if ready || live {
                "enrollment-discard"
            } else {
                "enrollment-discard-hidden"
            },
        );
        data.set(
            "discard_label",
            if live && running {
                localizer.text("ui-fingerprint-cancel")
            } else if ready {
                localizer.text("ui-enroll-discard")
            } else {
                String::new()
            },
        );
        data.set(
            "progress_class",
            if ready {
                "enrollment-field-hidden"
            } else {
                "status"
            },
        );
        data.set(
            "progress",
            if capture.required > 0 {
                format!("{} / {}", capture.accepted, capture.required)
            } else if capture.percent > 0.0 {
                format!("{:.0}%", capture.percent)
            } else {
                String::new()
            },
        );
        data.set(
            "instruction",
            localizer.text(self.completion.get().unwrap_or_else(|| {
                if self.cancelling.get() {
                    "ui-enroll-cancelling"
                } else if live && ready {
                    "ui-enroll-name-after-scan"
                } else if ready {
                    "ui-enroll-demo-ready"
                } else {
                    capture.message
                }
            })),
        );
        data.set(
            "event_class",
            if capture.retry && !ready {
                "value enrollment-event warning"
            } else {
                "value enrollment-event"
            },
        );
        crate::ui::shared::fill(
            self.screen.view(),
            [
                (
                    "fingerprint_print",
                    enrollment_print::view(capture.percent, motion),
                ),
                ("progress_ring", enrollment_ring::view(capture.percent)),
            ],
        )
    }
}
