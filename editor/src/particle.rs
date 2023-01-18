use crate::{
    scene::{EditorScene, Selection},
    Message,
};
use fyrox::{
    core::pool::Handle,
    engine::Engine,
    gui::{
        button::{ButtonBuilder, ButtonMessage},
        check_box::{CheckBoxBuilder, CheckBoxMessage},
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowMessage, WindowTitle},
        BuildContext, UiNode,
    },
    scene::{node::Node, particle_system::ParticleSystem},
};

pub struct ParticleSystemPreviewControlPanel {
    pub window: Handle<UiNode>,
    preview: Handle<UiNode>,
    play: Handle<UiNode>,
    pause: Handle<UiNode>,
    stop: Handle<UiNode>,
    rewind: Handle<UiNode>,
    particle_systems_state: Vec<(Handle<Node>, Node)>,
}

impl ParticleSystemPreviewControlPanel {
    pub fn new(ctx: &mut BuildContext) -> Self {
        let preview;
        let play;
        let pause;
        let stop;
        let rewind;
        let window = WindowBuilder::new(WidgetBuilder::new())
            .open(false)
            .with_title(WindowTitle::text("Particle System"))
            .with_content(
                GridBuilder::new(
                    WidgetBuilder::new()
                        .with_child({
                            preview =
                                CheckBoxBuilder::new(WidgetBuilder::new().on_row(0).on_column(0))
                                    .with_content(
                                        TextBuilder::new(WidgetBuilder::new())
                                            .with_text("Preview")
                                            .build(ctx),
                                    )
                                    .build(ctx);
                            preview
                        })
                        .with_child({
                            play = ButtonBuilder::new(
                                WidgetBuilder::new().on_row(0).on_column(1).with_width(60.0),
                            )
                            .with_text("Play")
                            .build(ctx);
                            play
                        })
                        .with_child({
                            pause = ButtonBuilder::new(
                                WidgetBuilder::new().on_row(0).on_column(2).with_width(60.0),
                            )
                            .with_text("Pause")
                            .build(ctx);
                            pause
                        })
                        .with_child({
                            stop = ButtonBuilder::new(
                                WidgetBuilder::new().on_row(0).on_column(3).with_width(60.0),
                            )
                            .with_text("Stop")
                            .build(ctx);
                            stop
                        })
                        .with_child({
                            rewind = ButtonBuilder::new(
                                WidgetBuilder::new().on_row(0).on_column(4).with_width(60.0),
                            )
                            .with_text("Rewind")
                            .build(ctx);
                            rewind
                        }),
                )
                .add_row(Row::stretch())
                .add_column(Column::stretch())
                .add_column(Column::stretch())
                .add_column(Column::stretch())
                .add_column(Column::stretch())
                .add_column(Column::stretch())
                .build(ctx),
            )
            .build(ctx);

        Self {
            window,
            play,
            pause,
            stop,
            rewind,
            preview,
            particle_systems_state: Default::default(),
        }
    }

    pub fn handle_message(
        &mut self,
        message: &Message,
        editor_scene: &mut EditorScene,
        engine: &mut Engine,
    ) {
        if let Message::DoSceneCommand(_) | Message::UndoSceneCommand | Message::RedoSceneCommand =
            message
        {
            self.leave_preview_mode(editor_scene, engine);
        }

        if let Message::SelectionChanged { .. } = message {
            let scene = &engine.scenes[editor_scene.scene];
            if let Selection::Graph(ref selection) = editor_scene.selection {
                let any_particle_system_selected = selection
                    .nodes
                    .iter()
                    .any(|n| scene.graph.try_get_of_type::<ParticleSystem>(*n).is_some());
                if any_particle_system_selected {
                    engine.user_interface.send_message(WindowMessage::open(
                        self.window,
                        MessageDirection::ToWidget,
                        false,
                    ));
                } else {
                    engine.user_interface.send_message(WindowMessage::close(
                        self.window,
                        MessageDirection::ToWidget,
                    ));
                }
            }
        }
    }

    fn enter_preview_mode(&mut self, editor_scene: &mut EditorScene, engine: &mut Engine) {
        assert!(self.particle_systems_state.is_empty());

        let scene = &engine.scenes[editor_scene.scene];
        let node_overrides = editor_scene.graph_switches.node_overrides.as_mut().unwrap();

        if let Selection::Graph(ref new_graph_selection) = editor_scene.selection {
            // Enable particle systems from new selection.
            for &node_handle in &new_graph_selection.nodes {
                if scene
                    .graph
                    .try_get_of_type::<ParticleSystem>(node_handle)
                    .is_some()
                {
                    self.particle_systems_state
                        .push((node_handle, scene.graph[node_handle].clone_box()));

                    assert!(node_overrides.insert(node_handle));
                }
            }
        }
    }

    pub fn leave_preview_mode(&mut self, editor_scene: &mut EditorScene, engine: &mut Engine) {
        let scene = &mut engine.scenes[editor_scene.scene];
        let node_overrides = editor_scene.graph_switches.node_overrides.as_mut().unwrap();

        for (particle_system_handle, original) in self.particle_systems_state.drain(..) {
            scene.graph[particle_system_handle] = original;

            assert!(node_overrides.remove(&particle_system_handle));
        }
    }

    pub fn handle_ui_message(
        &mut self,
        message: &UiMessage,
        editor_scene: &mut EditorScene,
        engine: &mut Engine,
    ) {
        if let Selection::Graph(ref selection) = editor_scene.selection {
            if let Some(ButtonMessage::Click) = message.data() {
                let scene = &mut engine.scenes[editor_scene.scene];

                for &node in &selection.nodes {
                    if let Some(particle_system) =
                        scene.graph.try_get_mut_of_type::<ParticleSystem>(node)
                    {
                        if message.destination() == self.play {
                            particle_system.play(true);
                        } else if message.destination() == self.pause {
                            particle_system.play(false);
                        } else if message.destination() == self.stop {
                            particle_system.play(false);
                            particle_system.clear_particles();
                        } else if message.destination() == self.rewind {
                            particle_system.clear_particles();
                        }
                    }
                }
            } else if let Some(CheckBoxMessage::Check(Some(value))) = message.data() {
                if message.destination() == self.preview
                    && message.direction() == MessageDirection::FromWidget
                {
                    if *value {
                        self.enter_preview_mode(editor_scene, engine);
                    } else {
                        self.leave_preview_mode(editor_scene, engine);
                    }
                }
            }
        }
    }
}