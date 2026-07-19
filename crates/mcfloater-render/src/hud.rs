use crate::FaceStatus;
use bevy::prelude::*;
use mcfloater_core::RuntimeState;

#[derive(Component)]
pub struct HudRoot;

#[derive(Component)]
pub struct HudStateText;

#[derive(Component)]
pub struct HudBrainText;

#[derive(Component)]
pub struct HudCaptionText;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // Top bar
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    HudStateText,
                    Text::new("STATE: IDLE"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.2, 1.0, 0.85)),
                ));
                top.spawn((
                    HudBrainText,
                    Text::new("BRAIN: …"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.8, 0.9)),
                ));
            });

            // Bottom caption + help
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|bot| {
                bot.spawn((
                    HudCaptionText,
                    Text::new(""),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.95, 0.7)),
                ));
                bot.spawn((
                    Text::new("Space = speak   A = ask brain   Esc = quit"),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.65, 0.7)),
                ));
            });
        });
}

pub fn update_hud(
    status: Res<FaceStatus>,
    mut state_q: Query<&mut Text, (With<HudStateText>, Without<HudBrainText>, Without<HudCaptionText>)>,
    mut brain_q: Query<&mut Text, (With<HudBrainText>, Without<HudStateText>, Without<HudCaptionText>)>,
    mut cap_q: Query<&mut Text, (With<HudCaptionText>, Without<HudStateText>, Without<HudBrainText>)>,
    mut state_color: Query<&mut TextColor, (With<HudStateText>, Without<HudBrainText>)>,
    mut brain_color: Query<&mut TextColor, (With<HudBrainText>, Without<HudStateText>)>,
) {
    if !status.is_changed() {
        return;
    }

    if let Ok(mut text) = state_q.get_single_mut() {
        *text = Text::new(format!("STATE: {}", state_label(status.state)));
    }
    if let Ok(mut color) = state_color.get_single_mut() {
        color.0 = state_color_for(status.state);
    }
    if let Ok(mut text) = brain_q.get_single_mut() {
        *text = Text::new(status.brain_detail.clone());
    }
    if let Ok(mut color) = brain_color.get_single_mut() {
        color.0 = if status.brain_ok {
            Color::srgb(0.3, 1.0, 0.5)
        } else {
            Color::srgb(1.0, 0.4, 0.3)
        };
    }
    if let Ok(mut text) = cap_q.get_single_mut() {
        *text = Text::new(status.caption.clone());
    }
}

fn state_label(s: RuntimeState) -> &'static str {
    match s {
        RuntimeState::Idle => "IDLE",
        RuntimeState::Listening => "LISTENING",
        RuntimeState::Thinking => "THINKING",
        RuntimeState::Speaking => "SPEAKING",
    }
}

fn state_color_for(s: RuntimeState) -> Color {
    match s {
        RuntimeState::Idle => Color::srgb(0.2, 1.0, 0.85),
        RuntimeState::Listening => Color::srgb(0.4, 0.8, 1.0),
        RuntimeState::Thinking => Color::srgb(1.0, 0.85, 0.2),
        RuntimeState::Speaking => Color::srgb(1.0, 0.35, 0.75),
    }
}
