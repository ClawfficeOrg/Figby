use super::lighting::{Light, Scene};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

#[derive(Clone)]
pub struct LightPanel {
    pub selected_index: usize,
    pub show_help: bool,
}

impl LightPanel {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            show_help: false,
        }
    }

    pub fn set_show_help(&mut self, v: bool) {
        self.show_help = v;
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_selected(&mut self, idx: usize, scene: &Scene) {
        if scene.lights.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = idx.min(scene.lights.len() - 1);
        }
    }

    pub fn adjust_intensity(scene: &mut Scene, idx: usize, delta: f32) {
        if idx >= scene.lights.len() {
            return;
        }
        match &mut scene.lights[idx] {
            Light::Ambient { intensity, .. }
            | Light::Directional { intensity, .. }
            | Light::Point { intensity, .. } => {
                *intensity = (*intensity + delta).clamp(0.0, 1.0);
            }
        }
    }

    pub fn light_type_str(scene: &Scene, idx: usize) -> Option<&'static str> {
        scene.lights.get(idx).map(|l| match l {
            Light::Ambient { .. } => "Ambient",
            Light::Directional { .. } => "Directional",
            Light::Point { .. } => "Point",
        })
    }

    pub fn light_intensity(scene: &Scene, idx: usize) -> Option<f32> {
        scene.lights.get(idx).map(|l| match l {
            Light::Ambient { intensity, .. }
            | Light::Directional { intensity, .. }
            | Light::Point { intensity, .. } => *intensity,
        })
    }

    /// Build the light-list `Line`s shared by the toolbox-area `LightPanel`
    /// render and the side-panel Props tab's Lighting entry.
    pub fn build_lines(
        scene: &Scene,
        selected_index: usize,
        theme: &super::theme::Theme,
    ) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();
        for (i, light) in scene.lights.iter().enumerate() {
            let prefix = if i == selected_index {
                " \u{25b6} "
            } else {
                "   "
            };
            let label = match light {
                Light::Ambient { intensity, .. } => format!("Amb  {:.2}", intensity),
                Light::Directional { intensity, .. } => format!("Dir  {:.2}", intensity),
                Light::Point {
                    intensity,
                    position,
                    ..
                } => format!(
                    "Pnt  {:.2} ({},{})",
                    intensity, position.0 as u16, position.1 as u16
                ),
            };
            let style = if i == selected_index {
                Style::default()
                    .fg(theme.general.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.general.secondary)
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, label),
                style,
            )));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                " (no lights) ",
                Style::default().fg(theme.general.secondary),
            )));
        }

        lines
    }

    pub fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        scene: &Option<Scene>,
        theme: &super::theme::Theme,
    ) {
        let block = Block::default()
            .title(" Lights ")
            .borders(super::layout::toolbox_list_borders())
            .style(Style::default().fg(theme.general.secondary));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let scene = match scene {
            Some(s) => s,
            None => return,
        };

        let mut lines = Self::build_lines(scene, self.selected_index, theme);

        if self.show_help {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled(
                " Keys: ",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(" Esc=exit")));
            lines.push(Line::from(Span::raw(" ↑/↓=select")));
            lines.push(Line::from(Span::raw(" ←/→=move")));
            lines.push(Line::from(Span::raw(" Sh+↑↓=v-move")));
            lines.push(Line::from(Span::raw(" +/-=intensity")));
            lines.push(Line::from(Span::raw(" A=ambient")));
            lines.push(Line::from(Span::raw(" D=directional")));
            lines.push(Line::from(Span::raw(" P=point")));
            lines.push(Line::from(Span::raw(" Del=remove")));
            self.show_help = false;
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }
}

impl Default for LightPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::lighting::{Attenuation, LightTarget, Rgb};

    #[test]
    fn test_light_panel_empty() {
        let panel = LightPanel::new();
        let scene = Scene::new();
        assert_eq!(panel.selected_index(), 0);
        assert!(scene.lights.is_empty());
    }

    #[test]
    fn test_light_panel_single() {
        let mut scene = Scene::new();
        scene.add_light(Light::Ambient {
            intensity: 0.7,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });
        assert_eq!(LightPanel::light_type_str(&scene, 0), Some("Ambient"));
        assert!((LightPanel::light_intensity(&scene, 0).unwrap() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_light_panel_selection() {
        let mut scene = Scene::new();
        scene.add_light(Light::Ambient {
            intensity: 0.5,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });
        scene.add_light(Light::Directional {
            direction: (0.0, 0.0, 1.0),
            intensity: 0.8,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });

        let mut panel = LightPanel::new();
        assert_eq!(panel.selected_index(), 0);
        panel.set_selected(1, &scene);
        assert_eq!(panel.selected_index(), 1);
        panel.set_selected(5, &scene);
        assert_eq!(panel.selected_index(), 1);
    }

    #[test]
    fn test_light_panel_multiple() {
        let mut scene = Scene::new();
        scene.add_light(Light::Ambient {
            intensity: 0.5,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });
        scene.add_light(Light::Directional {
            direction: (0.0, 0.0, 1.0),
            intensity: 0.8,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });
        scene.add_light(Light::Point {
            position: (5.0, 3.0, 5.0),
            intensity: 0.9,
            color: Rgb(255, 255, 255),
            attenuation: Attenuation::default(),
            target: LightTarget::default(),
        });

        assert_eq!(scene.lights.len(), 3);
        assert_eq!(LightPanel::light_type_str(&scene, 0), Some("Ambient"));
        assert_eq!(LightPanel::light_type_str(&scene, 1), Some("Directional"));
        assert_eq!(LightPanel::light_type_str(&scene, 2), Some("Point"));
    }

    #[test]
    fn test_light_panel_adjust_intensity() {
        let mut scene = Scene::new();
        scene.add_light(Light::Ambient {
            intensity: 0.5,
            color: Rgb(255, 255, 255),
            target: LightTarget::default(),
        });

        LightPanel::adjust_intensity(&mut scene, 0, 0.2);
        assert!((LightPanel::light_intensity(&scene, 0).unwrap() - 0.7).abs() < 0.001);

        LightPanel::adjust_intensity(&mut scene, 0, 0.5);
        assert!((LightPanel::light_intensity(&scene, 0).unwrap() - 1.0).abs() < 0.001);

        LightPanel::adjust_intensity(&mut scene, 0, -0.8);
        assert!((LightPanel::light_intensity(&scene, 0).unwrap() - 0.2).abs() < 0.001);
    }
}
