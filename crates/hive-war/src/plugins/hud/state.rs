use bevy::prelude::*;

#[derive(Resource, Default)]
pub(super) struct HudState {
    selected_base: Option<Entity>,
    pending_action: Option<PendingHudAction>,
    notice: Option<HudNotice>,
    pub(super) dev_overlay_open: bool,
    #[allow(dead_code)]
    pub(super) intel_overlay_open: bool,
}

impl HudState {
    pub(super) fn selected_base(&self) -> Option<Entity> {
        self.selected_base
    }

    pub(super) fn pending_action(&self) -> Option<PendingHudAction> {
        self.pending_action
    }

    pub(super) fn notice(&self) -> Option<&HudNotice> {
        self.notice.as_ref()
    }

    pub(super) fn reset_for_match_end(&mut self) {
        self.selected_base = None;
        self.pending_action = None;
        self.notice = None;
    }

    pub(super) fn drop_missing_selection(&mut self) {
        self.selected_base = None;
    }

    pub(super) fn select_base(&mut self, next_selected: Option<Entity>) {
        if next_selected != self.selected_base || next_selected.is_none() {
            self.pending_action = None;
        }
        if next_selected != self.selected_base {
            self.notice = None;
        }
        self.selected_base = next_selected;
    }

    pub(super) fn arm_action(&mut self, action: PendingHudAction) {
        self.pending_action = Some(action);
        self.notice = None;
    }

    pub(super) fn clear_pending(&mut self) {
        self.pending_action = None;
    }

    pub(super) fn resolve_pending(&mut self) {
        self.pending_action = None;
        self.notice = None;
    }

    pub(super) fn show_warning(&mut self, text: impl Into<String>) {
        self.notice = Some(HudNotice::warning(text));
    }

    pub(super) fn cancel_action(&mut self, action: PendingHudAction, reason: impl Into<String>) {
        self.pending_action = None;
        self.notice = Some(HudNotice::warning(format!(
            "{} canceled: {}",
            action.label(),
            reason.into()
        )));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingHudAction {
    UpgradeBase(Entity),
    RepairBase(Entity),
    GlobalExpansion,
}

impl PendingHudAction {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::UpgradeBase(_) => "Base upgrade",
            Self::RepairBase(_) => "Base repair",
            Self::GlobalExpansion => "Global expansion",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct HudNotice {
    pub(super) text: String,
}

impl HudNotice {
    pub(super) fn warning(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}
