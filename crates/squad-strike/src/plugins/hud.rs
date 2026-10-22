#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use swarm_model::{GeneticTraits, Unit};

use std::collections::BTreeMap;

use crate::plugins::{
    art::{HudIcon, icon_widget},
    combat::GameOver,
    enemies::{MAX_WAVES_PER_RUN, WaveState},
    evolution::EvolutionState,
    game_flow::RestartRequested,
    meta_progression::{MetaBranch, MetaEvolutionNode, MetaNodeKind, MetaProgression},
    meta_progression::{MetaRunState, NODE_ASSIMILATION},
    skills::{ChooseRunSkill, RunSkillKind, RunSkills, SkillDraftState},
    squad::{CaptainAnnouncement, SquadCaptain, SquadMember},
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EvolutionLabState>()
            .add_systems(EguiPrimaryContextPass, hud_system);
    }
}

#[derive(Resource)]
struct EvolutionLabState {
    selected_node: &'static str,
}

impl Default for EvolutionLabState {
    fn default() -> Self {
        Self {
            selected_node: "vitality_matrix",
        }
    }
}

fn hud_system(
    mut ctxs: EguiContexts,
    mut restart: MessageWriter<RestartRequested>,
    mut choose_skill: MessageWriter<ChooseRunSkill>,
    mut meta: ResMut<MetaProgression>,
    mut lab: ResMut<EvolutionLabState>,
    cfg: Res<swarm_model::GameConfig>,
    run: Res<MetaRunState>,
    wave: Res<WaveState>,
    evo: Res<EvolutionState>,
    game_over: Res<GameOver>,
    draft: Res<SkillDraftState>,
    skills: Res<RunSkills>,
    announcement: Res<CaptainAnnouncement>,
    players: Query<(&Unit, &GeneticTraits), With<SquadMember>>,
    captain: Query<(&Unit, &GeneticTraits), With<SquadCaptain>>,
) {
    let Ok(ctx) = ctxs.ctx_mut() else {
        return;
    };

    let living: Vec<(&Unit, &GeneticTraits)> =
        players.iter().filter(|(u, _)| !u.is_dead()).collect();
    let avg = average_traits(&living);

    egui::TopBottomPanel::top("squad_strike_top_hud")
        .frame(top_frame())
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "Wave",
                        &format!("{}/{}", wave.wave.max(1), MAX_WAVES_PER_RUN),
                        egui::Color32::from_rgb(123, 214, 255),
                        Some(HudIcon::WaveAlert),
                    );
                    pill(
                        ui,
                        "Next",
                        &next_wave_label(&wave, &draft),
                        egui::Color32::from_rgb(145, 205, 255),
                        None,
                    );
                    if let Some(edge) = wave.incoming_edge {
                        pill(
                            ui,
                            "Breach",
                            edge.label(),
                            egui::Color32::from_rgb(255, 156, 96),
                            None,
                        );
                    }
                    pill(
                        ui,
                        "Kills",
                        &wave.kills.to_string(),
                        egui::Color32::from_rgb(130, 239, 162),
                        None,
                    );
                    pill(
                        ui,
                        "Cap Lv",
                        &evo.captain_level.max(1).to_string(),
                        egui::Color32::from_rgb(255, 210, 102),
                        None,
                    );
                    pill(
                        ui,
                        "Units",
                        &living.len().to_string(),
                        egui::Color32::from_rgb(196, 208, 255),
                        None,
                    );
                    pill(
                        ui,
                        "Ready",
                        &evo.ready_units.to_string(),
                        egui::Color32::from_rgb(255, 122, 122),
                        None,
                    );
                    pill(
                        ui,
                        "Split",
                        &evo.split_units.to_string(),
                        egui::Color32::from_rgb(188, 157, 255),
                        None,
                    );
                    pill(
                        ui,
                        "Cells",
                        &meta.banked_cells().to_string(),
                        egui::Color32::from_rgb(255, 181, 94),
                        Some(HudIcon::EnergyCell),
                    );
                    pill(
                        ui,
                        "Run",
                        &run.pending_cells.to_string(),
                        egui::Color32::from_rgb(255, 226, 126),
                        Some(HudIcon::RepairKit),
                    );
                });
                ui.add_space(ui.available_width().max(0.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    stat_chip(ui, "APS", avg.attack_speed);
                    stat_chip(ui, "ARM", avg.armor);
                    stat_chip(ui, "RNG", avg.sensor_range);
                    stat_chip(ui, "SPD", avg.speed);
                    stat_chip(ui, "ATK", avg.attack);
                    stat_chip(ui, "HP", avg.max_health);
                });
            });
        });

    egui::TopBottomPanel::bottom("squad_strike_bottom_hud")
        .frame(bottom_frame())
        .show(ctx, |ui| {
            if let Some(text) = announcement.active_text() {
                ui.label(
                    egui::RichText::new(text)
                        .strong()
                        .color(egui::Color32::from_rgb(142, 242, 199)),
                );
                ui.add_space(4.0);
            }

            if let Ok((captain_unit, captain_traits)) = captain.single() {
                let hp_ratio = (captain_unit.current_health / captain_traits.max_health.max(1.0))
                    .clamp(0.0, 1.0);
                let xp_ratio = if captain_unit.is_max_level_with(cfg.max_unit_level) {
                    1.0
                } else {
                    (captain_unit.xp / captain_unit.xp_to_next.max(1.0)).clamp(0.0, 1.0)
                };
                ui.horizontal(|ui| {
                    icon_widget(ui, Some(HudIcon::CaptainBadge), 22.0);
                    ui.label(
                        egui::RichText::new("Captain")
                            .strong()
                            .color(egui::Color32::from_rgb(162, 255, 214)),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "Lv {}  |  {:.0} / {:.0}",
                            captain_unit.level,
                            captain_unit.current_health,
                            captain_traits.max_health
                        ))
                        .monospace()
                        .color(egui::Color32::from_rgb(220, 247, 231)),
                    );
                });
                ui.add(
                    egui::ProgressBar::new(hp_ratio)
                        .desired_width(ui.available_width())
                        .fill(egui::Color32::from_rgb(86, 226, 156))
                        .text(format!("{:.0}% integrity", hp_ratio * 100.0)),
                );
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    icon_widget(ui, Some(HudIcon::HealthOrb), 16.0);
                    ui.label(
                        egui::RichText::new("Captain integrity")
                            .small()
                            .color(egui::Color32::from_rgb(166, 226, 196)),
                    );
                });
                ui.add_space(4.0);
                ui.add(
                    egui::ProgressBar::new(xp_ratio)
                        .desired_width(ui.available_width())
                        .fill(egui::Color32::from_rgb(104, 148, 255))
                        .text(if captain_unit.is_max_level_with(cfg.max_unit_level) {
                            if evo.ready_units > 0 {
                                "Max level · ready to split".into()
                            } else {
                                "Max level".into()
                            }
                        } else {
                            format!("XP {:.0} / {:.0}", captain_unit.xp, captain_unit.xp_to_next)
                        }),
                );
            } else if !living.is_empty() {
                ui.label(
                    egui::RichText::new("Reassigning captain...")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 214, 125)),
                );
            } else {
                ui.label(
                    egui::RichText::new("Squad wiped out")
                        .strong()
                        .color(egui::Color32::from_rgb(255, 126, 126)),
                );
            }
        });

    if game_over.active {
        show_evolution_lab(
            ctx,
            &mut restart,
            &mut meta,
            &mut lab,
            &run,
            &wave,
            &game_over,
        );
    }

    if draft.active && !game_over.active {
        egui::Area::new("squad_strike_skill_draft".into())
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(6, 10, 16, 236))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(112, 180, 255),
                    ))
                    .corner_radius(18.0)
                    .inner_margin(egui::Margin::symmetric(18, 16))
                    .show(ui, |ui| {
                        ui.set_max_width(760.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Wave {} cleared", draft.cleared_wave))
                                    .small()
                                    .color(egui::Color32::from_rgb(140, 184, 234)),
                            );
                            ui.label(
                                egui::RichText::new("Choose 1 Upgrade")
                                    .heading()
                                    .strong()
                                    .color(egui::Color32::from_rgb(227, 241, 255)),
                            );
                            ui.add_space(10.0);
                        });
                        ui.horizontal_top(|ui| {
                            for kind in draft.choices.iter().flatten().copied() {
                                skill_choice_card(
                                    ui,
                                    kind,
                                    skills.stacks_for(kind),
                                    &mut choose_skill,
                                );
                            }
                        });
                    });
            });
    }
}

fn show_evolution_lab(
    ctx: &egui::Context,
    restart: &mut MessageWriter<RestartRequested>,
    meta: &mut MetaProgression,
    lab: &mut EvolutionLabState,
    run: &MetaRunState,
    wave: &WaveState,
    game_over: &GameOver,
) {
    let screen = ctx.content_rect();
    let size = egui::vec2(
        (screen.width() - 44.0).clamp(880.0, 1180.0),
        (screen.height() - 72.0).clamp(450.0, 720.0),
    );
    let pos = screen.center() - size * 0.5;
    let accent = if game_over.victory {
        egui::Color32::from_rgb(113, 232, 167)
    } else {
        egui::Color32::from_rgb(255, 91, 91)
    };

    egui::Area::new("squad_strike_evolution_lab".into())
        .fixed_pos(pos)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(7, 10, 15, 238))
                .stroke(egui::Stroke::new(1.0_f32, accent.gamma_multiply(0.8)))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(16, 14))
                .show(ui, |ui| {
                    ui.set_min_size(size);
                    show_lab_header(ui, meta, run, wave, game_over, accent);
                    ui.add_space(12.0);
                    ui.horizontal_top(|ui| {
                        show_lab_left_rail(ui, restart, meta, run);
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_width((size.x - 500.0).max(360.0));
                            show_evolution_tree(ui, meta, lab);
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_width(300.0);
                            show_evolution_inspector(ui, meta, lab);
                        });
                    });
                });
        });
}

fn show_lab_header(
    ui: &mut egui::Ui,
    meta: &MetaProgression,
    run: &MetaRunState,
    wave: &WaveState,
    game_over: &GameOver,
    accent: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(if game_over.victory {
                    "RUN CLEARED"
                } else {
                    "EVOLUTION LAB"
                })
                .heading()
                .strong()
                .color(accent),
            );
            ui.label(
                egui::RichText::new(if game_over.victory {
                    format!("All {} waves cleared.", MAX_WAVES_PER_RUN)
                } else {
                    format!("Squad wiped on wave {}.", wave.wave.max(1))
                })
                .color(egui::Color32::from_rgb(209, 221, 238)),
            );
        });
        ui.add_space(ui.available_width().max(0.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            pill(
                ui,
                "Lifetime",
                &meta.total_cells_earned().to_string(),
                egui::Color32::from_rgb(151, 206, 255),
                None,
            );
            pill(
                ui,
                "Bank",
                &meta.banked_cells().to_string(),
                egui::Color32::from_rgb(255, 181, 94),
                Some(HudIcon::EnergyCell),
            );
            pill(
                ui,
                "Recovered",
                &run.banked_on_death.to_string(),
                egui::Color32::from_rgb(255, 226, 126),
                Some(HudIcon::RepairKit),
            );
        });
    });
}

fn show_lab_left_rail(
    ui: &mut egui::Ui,
    restart: &mut MessageWriter<RestartRequested>,
    meta: &MetaProgression,
    run: &MetaRunState,
) {
    ui.vertical(|ui| {
        ui.set_width(150.0);
        let restart_button = egui::Button::new(
            egui::RichText::new("Restart")
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(egui::Color32::from_rgb(47, 121, 255))
        .min_size(egui::vec2(136.0, 38.0));
        if ui.add(restart_button).clicked() {
            restart.write(RestartRequested);
        }

        ui.add_space(12.0);
        side_metric(
            ui,
            "Cells",
            meta.banked_cells().to_string(),
            HudIcon::EnergyCell,
        );
        side_metric(
            ui,
            "This run",
            run.banked_on_death.to_string(),
            HudIcon::RepairKit,
        );
        side_metric(
            ui,
            "Nodes",
            format!(
                "{}/{}",
                meta.unlocked_node_count(),
                MetaProgression::nodes().len()
            ),
            HudIcon::CaptainBadge,
        );
        side_metric(
            ui,
            "Ranks",
            meta.total_purchased_ranks().to_string(),
            HudIcon::ShieldEmitter,
        );

        if let Some(text) = meta.last_notice() {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(text)
                    .small()
                    .color(egui::Color32::from_rgb(150, 236, 186)),
            );
        }
    });
}

fn side_metric(ui: &mut egui::Ui, label: &str, value: String, icon: HudIcon) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 20, 28, 180))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(150, 210, 255, 42),
        ))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon_widget(ui, Some(icon), 18.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .small()
                            .color(egui::Color32::from_rgb(142, 157, 179)),
                    );
                    ui.label(
                        egui::RichText::new(value)
                            .monospace()
                            .strong()
                            .color(egui::Color32::from_rgb(230, 241, 255)),
                    );
                });
            });
        });
    ui.add_space(7.0);
}

fn show_evolution_tree(ui: &mut egui::Ui, meta: &MetaProgression, lab: &mut EvolutionLabState) {
    let selected_exists = MetaProgression::nodes()
        .iter()
        .any(|node| node.id == lab.selected_node);
    if !selected_exists {
        lab.selected_node = MetaProgression::nodes()
            .first()
            .map(|node| node.id)
            .unwrap_or("vitality_matrix");
    }

    egui::ScrollArea::vertical()
        .id_salt("squad_strike_evolution_tree")
        .show(ui, |ui| {
            let mut node_rects = BTreeMap::new();
            for branch in MetaBranch::ALL {
                ui.horizontal(|ui| {
                    icon_widget(ui, Some(branch_icon(branch)), 18.0);
                    ui.label(
                        egui::RichText::new(branch.title())
                            .strong()
                            .color(branch_color(branch)),
                    );
                });
                ui.add_space(5.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for node in MetaProgression::nodes()
                        .iter()
                        .filter(|node| node.branch == branch)
                    {
                        let response =
                            evolution_node_button(ui, meta, *node, lab.selected_node == node.id);
                        node_rects.insert(node.id, response.rect);
                        if response.clicked() {
                            lab.selected_node = node.id;
                        }
                    }
                });
                ui.add_space(13.0);
            }
            draw_evolution_links(ui, meta, &node_rects, lab.selected_node);
        });
}

fn draw_evolution_links(
    ui: &egui::Ui,
    meta: &MetaProgression,
    node_rects: &BTreeMap<&'static str, egui::Rect>,
    selected_node: &'static str,
) {
    let painter = ui.painter();
    for node in MetaProgression::nodes() {
        let Some(to_rect) = node_rects.get(node.id) else {
            continue;
        };
        for prereq in node.prereqs {
            let Some(from_rect) = node_rects.get(prereq.node_id) else {
                continue;
            };
            let met = meta.node_rank(prereq.node_id) >= prereq.rank;
            let highlighted = node.id == selected_node || prereq.node_id == selected_node;
            let color = if met {
                branch_color(node.branch).gamma_multiply(if highlighted { 0.9 } else { 0.5 })
            } else {
                egui::Color32::from_rgba_unmultiplied(100, 117, 142, 96)
            };
            let width: f32 = if highlighted { 2.0 } else { 1.25 };
            let stroke = egui::Stroke::new(width, color);
            let start = from_rect.right_center() + egui::vec2(4.0, 0.0);
            let end = to_rect.left_center() - egui::vec2(4.0, 0.0);
            let mid_x = (start.x + end.x) * 0.5;
            let a = egui::pos2(mid_x, start.y);
            let b = egui::pos2(mid_x, end.y);
            painter.line_segment([start, a], stroke);
            painter.line_segment([a, b], stroke);
            painter.line_segment([b, end], stroke);
            painter.circle_filled(end, if met { 2.5 } else { 2.0 }, color);
        }
    }
}

fn evolution_node_button(
    ui: &mut egui::Ui,
    meta: &MetaProgression,
    node: MetaEvolutionNode,
    selected: bool,
) -> egui::Response {
    let rank = meta.node_rank(node.id);
    let locked = !meta.prereqs_met(node);
    let maxed = meta.is_node_maxed(node);
    let ready = meta.can_purchase_node(node);
    let color = branch_color(node.branch);
    let fill = if selected {
        color.gamma_multiply(0.28)
    } else if maxed {
        egui::Color32::from_rgba_unmultiplied(31, 50, 45, 232)
    } else if locked {
        egui::Color32::from_rgba_unmultiplied(25, 28, 34, 218)
    } else {
        egui::Color32::from_rgba_unmultiplied(16, 23, 32, 226)
    };
    let stroke_color = if selected || ready {
        color
    } else if maxed {
        egui::Color32::from_rgb(104, 214, 154)
    } else {
        egui::Color32::from_rgba_unmultiplied(124, 151, 184, 74)
    };
    let status = if maxed {
        "Complete".into()
    } else if locked {
        "Locked".into()
    } else if let Some(cost) = meta.next_cost(node) {
        format!("{cost} Cells")
    } else {
        "Ready".into()
    };
    let text_color = if locked {
        egui::Color32::from_rgb(126, 137, 154)
    } else {
        egui::Color32::from_rgb(229, 239, 255)
    };
    let marker = match node.kind {
        MetaNodeKind::Rank => "R",
        MetaNodeKind::Unlock => "U",
        MetaNodeKind::Keystone => "K",
    };
    let label = format!(
        "{marker}  {}\nLv {}/{}\n{}",
        node.title, rank, node.max_rank, status
    );
    ui.add_sized(
        egui::vec2(168.0, 78.0),
        egui::Button::new(egui::RichText::new(label).color(text_color))
            .fill(fill)
            .stroke(egui::Stroke::new(1.0_f32, stroke_color)),
    )
}

fn show_evolution_inspector(
    ui: &mut egui::Ui,
    meta: &mut MetaProgression,
    lab: &EvolutionLabState,
) {
    let node = MetaProgression::nodes()
        .iter()
        .find(|node| node.id == lab.selected_node)
        .copied()
        .unwrap_or_else(|| MetaProgression::nodes()[0]);
    let rank = meta.node_rank(node.id);
    let color = branch_color(node.branch);
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(12, 18, 27, 224))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.6)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(12, 12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon_widget(ui, Some(node_icon(node)), 32.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(node.title)
                            .strong()
                            .color(egui::Color32::from_rgb(238, 244, 255)),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            node.branch.title(),
                            node_kind_label(node.kind)
                        ))
                        .small()
                        .color(color),
                    );
                });
            });
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(node.description).color(egui::Color32::from_rgb(186, 201, 224)),
            );
            ui.add_space(10.0);
            detail_line(ui, "Current", meta.node_effect_summary(node));
            if let Some(next) = meta.next_node_effect_summary(node) {
                detail_line(ui, "Next", next);
            }
            detail_line(ui, "Rank", format!("{rank}/{}", node.max_rank));

            if !node.prereqs.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Prereqs")
                        .small()
                        .strong()
                        .color(egui::Color32::from_rgb(151, 173, 205)),
                );
                for prereq in node.prereqs {
                    let met = meta.node_rank(prereq.node_id) >= prereq.rank;
                    let color = if met {
                        egui::Color32::from_rgb(142, 232, 176)
                    } else {
                        egui::Color32::from_rgb(215, 150, 122)
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} Lv {}",
                            node_title(prereq.node_id),
                            prereq.rank
                        ))
                        .small()
                        .color(color),
                    );
                }
            }

            ui.add_space(12.0);
            if meta.is_node_maxed(node) {
                ui.label(
                    egui::RichText::new("Complete")
                        .strong()
                        .color(egui::Color32::from_rgb(142, 232, 176)),
                );
            } else if !meta.prereqs_met(node) {
                ui.label(
                    egui::RichText::new("Locked")
                        .strong()
                        .color(egui::Color32::from_rgb(215, 150, 122)),
                );
            } else if let Some(cost) = meta.next_cost(node) {
                let can_buy = meta.can_purchase_node(node);
                let button = egui::Button::new(
                    egui::RichText::new(format!("Buy {cost}"))
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(if can_buy {
                    egui::Color32::from_rgb(47, 121, 255)
                } else {
                    egui::Color32::from_rgb(66, 73, 86)
                })
                .min_size(egui::vec2(128.0, 34.0));
                if ui.add_enabled(can_buy, button).clicked() {
                    meta.purchase_node(node.id);
                }
            }
        });
}

fn detail_line(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .small()
                .color(egui::Color32::from_rgb(142, 157, 179)),
        );
        ui.add_space(ui.available_width().max(0.0));
        ui.label(
            egui::RichText::new(value)
                .small()
                .strong()
                .color(egui::Color32::from_rgb(226, 237, 252)),
        );
    });
}

fn node_title(node_id: &str) -> &'static str {
    MetaProgression::nodes()
        .iter()
        .find(|node| node.id == node_id)
        .map(|node| node.title)
        .unwrap_or("Unknown")
}

fn node_kind_label(kind: MetaNodeKind) -> &'static str {
    match kind {
        MetaNodeKind::Rank => "Rank",
        MetaNodeKind::Unlock => "Unlock",
        MetaNodeKind::Keystone => "Keystone",
    }
}

fn node_icon(node: MetaEvolutionNode) -> HudIcon {
    if node.id == NODE_ASSIMILATION {
        return HudIcon::EliteTarget;
    }
    branch_icon(node.branch)
}

fn branch_icon(branch: MetaBranch) -> HudIcon {
    match branch {
        MetaBranch::Body => HudIcon::HealthOrb,
        MetaBranch::Firepower => HudIcon::EliteTarget,
        MetaBranch::Mobility => HudIcon::SpeedBoost,
        MetaBranch::Evolution => HudIcon::CaptainBadge,
        MetaBranch::Salvage => HudIcon::EnergyCell,
    }
}

fn branch_color(branch: MetaBranch) -> egui::Color32 {
    match branch {
        MetaBranch::Body => egui::Color32::from_rgb(119, 236, 164),
        MetaBranch::Firepower => egui::Color32::from_rgb(255, 168, 94),
        MetaBranch::Mobility => egui::Color32::from_rgb(128, 206, 255),
        MetaBranch::Evolution => egui::Color32::from_rgb(191, 164, 255),
        MetaBranch::Salvage => egui::Color32::from_rgb(255, 216, 112),
    }
}

fn average_traits(units: &[(&Unit, &GeneticTraits)]) -> GeneticTraits {
    if units.is_empty() {
        return GeneticTraits::default();
    }
    let mut acc = GeneticTraits {
        max_health: 0.0,
        attack: 0.0,
        speed: 0.0,
        sensor_range: 0.0,
        armor: 0.0,
        attack_speed: 0.0,
    };
    for (_, traits) in units {
        acc.max_health += traits.max_health;
        acc.attack += traits.attack;
        acc.speed += traits.speed;
        acc.sensor_range += traits.sensor_range;
        acc.armor += traits.armor;
        acc.attack_speed += traits.attack_speed;
    }
    let inv = 1.0 / units.len() as f32;
    acc.max_health *= inv;
    acc.attack *= inv;
    acc.speed *= inv;
    acc.sensor_range *= inv;
    acc.armor *= inv;
    acc.attack_speed *= inv;
    acc
}

fn next_wave_label(wave: &WaveState, draft: &SkillDraftState) -> String {
    if wave.enemies_alive > 0 {
        return format!("{} left", wave.enemies_alive);
    }
    if wave.wave >= MAX_WAVES_PER_RUN {
        return "final".into();
    }
    if draft.active {
        return "pick".into();
    }
    format!("{:.0}s", wave.next_wave_timer.ceil().max(0.0))
}

fn pill(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32, icon: Option<HudIcon>) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 20, 28, 180))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.6)))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon_widget(ui, icon, 16.0);
                ui.label(
                    egui::RichText::new(label)
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.label(egui::RichText::new(value).strong().monospace().color(color));
            });
        });
}

fn stat_chip(ui: &mut egui::Ui, label: &str, value: f32) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(13, 18, 24, 170))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(150, 210, 255, 38),
        ))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(label)
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new(format!("{value:.1}"))
                        .monospace()
                        .strong()
                        .color(egui::Color32::from_rgb(223, 242, 255)),
                );
            });
        });
}

fn top_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(3, 7, 12, 185))
        .inner_margin(egui::Margin::symmetric(10, 7))
}

fn bottom_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(4, 10, 16, 188))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn skill_choice_card(
    ui: &mut egui::Ui,
    kind: RunSkillKind,
    current_stacks: u32,
    choose_skill: &mut MessageWriter<ChooseRunSkill>,
) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(15, 22, 31, 232))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(106, 167, 255, 96),
        ))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(12, 12))
        .show(ui, |ui| {
            ui.set_width(228.0);
            ui.vertical(|ui| {
                icon_widget(ui, Some(run_skill_icon(kind)), 30.0);
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(kind.title())
                        .strong()
                        .color(egui::Color32::from_rgb(234, 241, 255)),
                );
                ui.label(
                    egui::RichText::new(format!("Current Lv {}", current_stacks))
                        .small()
                        .color(egui::Color32::from_rgb(144, 171, 204)),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(kind.description(current_stacks))
                        .small()
                        .color(egui::Color32::from_rgb(197, 210, 226)),
                );
                ui.add_space(12.0);
                let button = egui::Button::new(
                    egui::RichText::new("Pick")
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(47, 121, 255))
                .min_size(egui::vec2(90.0, 30.0));
                if ui.add(button).clicked() {
                    choose_skill.write(ChooseRunSkill(kind));
                }
            });
        });
}

fn run_skill_icon(kind: RunSkillKind) -> HudIcon {
    match kind {
        RunSkillKind::AssimilationProtocol => HudIcon::EliteTarget,
        RunSkillKind::TriagePulse => HudIcon::HealthOrb,
        RunSkillKind::SalvageProtocol => HudIcon::SalvageScrap,
        RunSkillKind::TargetingRelay => HudIcon::EliteTarget,
        RunSkillKind::RapidCycler => HudIcon::ShieldEmitter,
        RunSkillKind::GuardFormation => HudIcon::CaptainBadge,
        RunSkillKind::SplitCatalyst => HudIcon::EnergyCell,
        RunSkillKind::MomentumDrive => HudIcon::SpeedBoost,
    }
}
