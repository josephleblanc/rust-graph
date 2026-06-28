#[cfg(feature = "bevy-render")]
mod render_app {
    use bevy::{
        camera::{CameraOutputMode, Viewport},
        color::Srgba,
        core_pipeline::tonemapping::Tonemapping,
        ecs::{event::EntityEvent, hierarchy::ChildSpawnerCommands},
        feathers::{FeathersPlugins, dark_theme::create_dark_theme, theme::UiTheme},
        input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel},
        picking::{
            Pickable,
            events::{Drag, DragEnd, Pointer, Press},
            hover::HoverMap,
        },
        post_process::bloom::Bloom,
        prelude::*,
        render::render_resource::BlendState,
        ui::{
            BackgroundGradient, ColorStop, ComputedNode, ComputedUiRenderTargetInfo,
            ComputedUiTargetCamera, Gradient, InterpolationColorSpace, LinearGradient,
            OverflowAxis, ScrollPosition, UiGlobalTransform, UiScale, UiSystems, UiTransform, Val2,
        },
        ui_widgets::{
            Slider, SliderOrientation, SliderPrecision, SliderRange, SliderStep, SliderThumb,
            SliderValue, TrackClick, ValueChange,
        },
    };
    use my_graph_core::{ForceLayoutSettings, Graph, GraphEdge, GraphNode};

    const PANEL_WIDTH: f32 = 320.0;
    const NODE_RADIUS: f32 = 0.16;
    const EDGE_RADIUS: f32 = 0.018;

    pub fn run() {
        App::new()
            .insert_resource(ClearColor(Color::srgb(0.015, 0.016, 0.020)))
            .insert_resource(GraphResource(Graph::sample()))
            .insert_resource(GraphPresetState::default())
            .insert_resource(PendingGraphPreset::default())
            .insert_resource(LayoutControls::default())
            .insert_resource(GraphStyle::default())
            .insert_resource(StartupDiagnostics::default())
            .insert_resource(UiTheme(create_dark_theme()))
            .add_plugins((
                DefaultPlugins.set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "My Graph".to_string(),
                        canvas: Some("#bevy".to_string()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
                FeathersPlugins,
            ))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    update_graph_camera_viewport,
                    send_scroll_events,
                    orbit_graph_camera,
                    handle_control_buttons,
                    apply_selected_preset.after(handle_control_buttons),
                    update_button_colors,
                    update_graph_selector,
                    update_graph_readouts,
                    sync_scalar_widgets,
                    sync_node_color_swatch,
                    update_graph_style,
                    update_slider_visuals,
                    update_color_slider_tracks,
                    update_node_color_plane_visuals,
                    update_scalar_readouts,
                    update_status_readouts,
                    apply_force_layout,
                    sync_edge_transforms.after(apply_force_layout),
                    rotate_graph_slowly,
                ),
            )
            .add_observer(handle_scalar_slider_change)
            .add_observer(on_scroll_handler)
            .add_observer(handle_node_color_plane_press)
            .add_observer(handle_node_color_plane_drag)
            .add_observer(handle_node_color_plane_drag_end)
            .add_systems(
                PostUpdate,
                log_startup_visual_state.after(UiSystems::PostLayout),
            )
            .run();
    }

    #[derive(Resource)]
    struct GraphResource(Graph);

    #[derive(Resource)]
    struct GraphPresetState {
        selected: GraphPreset,
        menu_open: bool,
    }

    impl Default for GraphPresetState {
        fn default() -> Self {
            Self {
                selected: GraphPreset::Sample,
                menu_open: false,
            }
        }
    }

    #[derive(Resource, Default)]
    struct PendingGraphPreset(Option<GraphPreset>);

    #[derive(Resource)]
    struct LayoutControls {
        settings: ForceLayoutSettings,
        simulation_running: bool,
        slow_rotation: bool,
    }

    impl Default for LayoutControls {
        fn default() -> Self {
            Self {
                settings: ForceLayoutSettings::default(),
                simulation_running: true,
                slow_rotation: true,
            }
        }
    }

    #[derive(Resource)]
    struct GraphStyle {
        node_color: Srgba,
        node_glow: f32,
        edge_color: Srgba,
        edge_glow: f32,
        edge_width: f32,
    }

    impl Default for GraphStyle {
        fn default() -> Self {
            Self {
                node_color: Srgba::new(0.36, 0.82, 0.95, 1.0),
                node_glow: 3.4,
                edge_color: Srgba::new(0.34, 0.45, 0.55, 0.72),
                edge_glow: 0.7,
                edge_width: 1.0,
            }
        }
    }

    #[derive(Resource)]
    struct GraphAssets {
        node_mesh: Handle<Mesh>,
        edge_mesh: Handle<Mesh>,
        node_material: Handle<StandardMaterial>,
        edge_material: Handle<StandardMaterial>,
    }

    #[derive(Resource, Default)]
    struct StartupDiagnostics {
        frames_seen: u32,
        printed: bool,
    }

    #[derive(Component)]
    struct GraphCamera;

    #[derive(Component)]
    struct OrbitCamera {
        target: Vec3,
        radius: f32,
        yaw: f32,
        pitch: f32,
    }

    #[derive(Component)]
    struct GraphRoot;

    #[derive(Component)]
    struct UiRoot;

    #[derive(Component)]
    struct SidePanel;

    #[derive(Component)]
    struct GraphViewportPane;

    #[derive(Component)]
    struct GraphNodeHandle {
        index: usize,
    }

    #[derive(Component)]
    struct GraphEdgeHandle {
        source: usize,
        target: usize,
    }

    #[derive(Component, Default)]
    struct Velocity(Vec3);

    #[derive(Component, Clone, Copy)]
    enum ControlAction {
        ToggleSimulation,
        ToggleRotation,
        ResetLayout,
        TogglePresetMenu,
        SelectPreset(GraphPreset),
    }

    #[derive(Component, Clone, Copy)]
    enum StatusReadout {
        Simulation,
        Rotation,
    }

    #[derive(Component, Clone, Copy)]
    enum GraphStat {
        Nodes,
        Edges,
    }

    #[derive(Component)]
    struct PresetLabel;

    #[derive(Component)]
    struct PresetDropdownList;

    #[derive(Clone, Copy)]
    enum LayoutField {
        Repulsion,
        Attraction,
        EdgeLength,
        Damping,
    }

    #[derive(Clone, Copy)]
    enum StyleField {
        NodeGlow,
        Width,
        Glow,
    }

    #[derive(Clone, Copy)]
    enum ColorChannel {
        Red,
        Green,
        Blue,
    }

    #[derive(Clone, Copy)]
    enum ScalarTarget {
        Layout(LayoutField),
        Style(StyleField),
        NodeColor(ColorChannel),
    }

    #[derive(Component, Clone, Copy)]
    struct ScalarControl(ScalarTarget);

    #[derive(Component, Clone, Copy)]
    struct ScalarReadout(ScalarTarget);

    #[derive(Clone, Copy)]
    struct ScalarSpec {
        default_value: f32,
        range: (f32, f32),
        step: f32,
        precision: i32,
        fill_color: Color,
    }

    #[derive(Clone, Copy)]
    struct ControlSection {
        title: &'static str,
        gap: f32,
        rows: &'static [ControlRow],
    }

    #[derive(Clone, Copy)]
    enum ControlRow {
        Scalar(&'static str, ScalarTarget),
        Toggle(&'static str, StatusReadout, ControlAction),
        NodeColorHeader,
        ColorPlane,
        ColorSlider(&'static str, ColorChannel),
    }

    const FORCE_ROWS: &[ControlRow] = &[
        ControlRow::Scalar("Repulsion", ScalarTarget::Layout(LayoutField::Repulsion)),
        ControlRow::Scalar("Attraction", ScalarTarget::Layout(LayoutField::Attraction)),
        ControlRow::Scalar("Edge length", ScalarTarget::Layout(LayoutField::EdgeLength)),
        ControlRow::Scalar("Damping", ScalarTarget::Layout(LayoutField::Damping)),
    ];

    const NODE_COLOR_ROWS: &[ControlRow] = &[
        ControlRow::NodeColorHeader,
        ControlRow::ColorPlane,
        ControlRow::ColorSlider("R", ColorChannel::Red),
        ControlRow::ColorSlider("G", ColorChannel::Green),
        ControlRow::ColorSlider("B", ColorChannel::Blue),
        ControlRow::Scalar("Glow", ScalarTarget::Style(StyleField::NodeGlow)),
    ];

    const EDGE_ROWS: &[ControlRow] = &[
        ControlRow::Scalar("Width", ScalarTarget::Style(StyleField::Width)),
        ControlRow::Scalar("Glow", ScalarTarget::Style(StyleField::Glow)),
    ];

    const RUNTIME_ROWS: &[ControlRow] = &[
        ControlRow::Toggle(
            "Simulation",
            StatusReadout::Simulation,
            ControlAction::ToggleSimulation,
        ),
        ControlRow::Toggle(
            "Orbit",
            StatusReadout::Rotation,
            ControlAction::ToggleRotation,
        ),
    ];

    const CONTROL_SECTIONS: &[ControlSection] = &[
        ControlSection {
            title: "Force model",
            gap: 10.0,
            rows: FORCE_ROWS,
        },
        ControlSection {
            title: "",
            gap: 8.0,
            rows: NODE_COLOR_ROWS,
        },
        ControlSection {
            title: "Edge appearance",
            gap: 8.0,
            rows: EDGE_ROWS,
        },
        ControlSection {
            title: "Runtime",
            gap: 10.0,
            rows: RUNTIME_ROWS,
        },
    ];

    #[derive(Component)]
    struct NodeColorPlane;

    #[derive(Component)]
    struct NodeColorPlaneInner;

    #[derive(Component)]
    struct NodeColorPlaneThumb;

    #[derive(Component)]
    struct NodeColorSwatch;

    #[derive(Component)]
    struct GraphSlider;

    #[derive(Component)]
    struct GraphSliderFill;

    #[derive(Component)]
    struct GraphSliderThumb;

    #[derive(Component, Clone, Copy)]
    struct ColorSliderTrack(ColorChannel);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum GraphPreset {
        Single,
        Pair,
        Sample,
    }

    impl GraphPreset {
        const ALL: [Self; 3] = [Self::Single, Self::Pair, Self::Sample];

        fn label(self) -> &'static str {
            match self {
                Self::Single => "Single node",
                Self::Pair => "Two nodes + edge",
                Self::Sample => "Sample graph",
            }
        }

        fn graph(self) -> Graph {
            match self {
                Self::Single => Graph::new(
                    vec![GraphNode::new(0, "Node", [0.0, 0.0, 0.0], 1.0)],
                    vec![],
                ),
                Self::Pair => Graph::new(
                    vec![
                        GraphNode::new(0, "Source", [-0.8, 0.0, 0.0], 1.0),
                        GraphNode::new(1, "Target", [0.8, 0.0, 0.0], 1.0),
                    ],
                    vec![GraphEdge::new(0, 1, 1.0)],
                ),
                Self::Sample => Graph::sample(),
            }
        }
    }

    impl LayoutField {
        fn value(self, settings: &ForceLayoutSettings) -> f32 {
            match self {
                Self::Repulsion => settings.repulsion,
                Self::Attraction => settings.attraction,
                Self::EdgeLength => settings.target_edge_length,
                Self::Damping => settings.damping,
            }
        }

        fn set(self, settings: &mut ForceLayoutSettings, value: f32) {
            let value = value.clamp(self.range().0, self.range().1);
            match self {
                Self::Repulsion => settings.repulsion = value,
                Self::Attraction => settings.attraction = value,
                Self::EdgeLength => settings.target_edge_length = value,
                Self::Damping => settings.damping = value,
            }
        }

        fn range(self) -> (f32, f32) {
            match self {
                Self::Repulsion => (0.1, 8.0),
                Self::Attraction => (0.05, 4.0),
                Self::EdgeLength => (0.4, 4.0),
                Self::Damping => (0.65, 0.98),
            }
        }

        fn step(self) -> f32 {
            match self {
                Self::Damping => 0.01,
                Self::Repulsion | Self::Attraction | Self::EdgeLength => 0.05,
            }
        }

        fn spec(self) -> ScalarSpec {
            let default = ForceLayoutSettings::default();
            ScalarSpec {
                default_value: self.value(&default),
                range: self.range(),
                step: self.step(),
                precision: 2,
                fill_color: Color::srgba(0.20, 0.64, 0.78, 0.95),
            }
        }
    }

    impl StyleField {
        fn value(self, style: &GraphStyle) -> f32 {
            match self {
                Self::NodeGlow => style.node_glow,
                Self::Width => style.edge_width,
                Self::Glow => style.edge_glow,
            }
        }

        fn set(self, style: &mut GraphStyle, value: f32) {
            let value = value.clamp(self.range().0, self.range().1);
            match self {
                Self::NodeGlow => style.node_glow = value,
                Self::Width => style.edge_width = value,
                Self::Glow => style.edge_glow = value,
            }
        }

        fn range(self) -> (f32, f32) {
            match self {
                Self::NodeGlow => (0.0, 6.0),
                Self::Width => (0.35, 4.0),
                Self::Glow => (0.0, 3.0),
            }
        }

        fn step(self) -> f32 {
            match self {
                Self::NodeGlow => 0.1,
                Self::Width | Self::Glow => 0.05,
            }
        }

        fn precision(self) -> i32 {
            match self {
                Self::NodeGlow => 1,
                Self::Width | Self::Glow => 2,
            }
        }

        fn spec(self) -> ScalarSpec {
            let default = GraphStyle::default();
            ScalarSpec {
                default_value: self.value(&default),
                range: self.range(),
                step: self.step(),
                precision: self.precision(),
                fill_color: Color::srgba(0.56, 0.72, 0.36, 0.95),
            }
        }
    }

    impl ColorChannel {
        fn value(self, color: Srgba) -> f32 {
            match self {
                Self::Red => color.red,
                Self::Green => color.green,
                Self::Blue => color.blue,
            }
        }

        fn set(self, color: &mut Srgba, value: f32) {
            let value = value.clamp(0.0, 1.0);
            match self {
                Self::Red => color.red = value,
                Self::Green => color.green = value,
                Self::Blue => color.blue = value,
            }
        }

        fn spec(self) -> ScalarSpec {
            ScalarSpec {
                default_value: self.value(GraphStyle::default().node_color),
                range: (0.0, 1.0),
                step: 0.01,
                precision: 2,
                fill_color: Color::WHITE,
            }
        }
    }

    impl ScalarTarget {
        fn spec(self) -> ScalarSpec {
            match self {
                Self::Layout(field) => field.spec(),
                Self::Style(field) => field.spec(),
                Self::NodeColor(channel) => channel.spec(),
            }
        }

        fn value(self, controls: &LayoutControls, style: &GraphStyle) -> f32 {
            match self {
                Self::Layout(field) => field.value(&controls.settings),
                Self::Style(field) => field.value(style),
                Self::NodeColor(channel) => channel.value(style.node_color),
            }
        }

        fn readout(self, controls: &LayoutControls, style: &GraphStyle) -> String {
            let precision = self.spec().precision as usize;
            format!("{:.*}", precision, self.value(controls, style))
        }
    }

    #[derive(EntityEvent, Debug)]
    #[entity_event(propagate, auto_propagate)]
    struct Scroll {
        entity: Entity,
        delta: Vec2,
    }

    type ColorPlaneInner<'a> = (
        &'a ComputedNode,
        &'a ComputedUiRenderTargetInfo,
        &'a UiGlobalTransform,
    );

    fn setup(
        mut commands: Commands,
        graph: Res<GraphResource>,
        style: Res<GraphStyle>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        let ui_camera = setup_cameras(&mut commands);
        let graph_assets = create_graph_assets(&style, &mut meshes, &mut materials);
        setup_graph(&mut commands, &graph.0, &style, &graph_assets);
        commands.insert_resource(graph_assets);
        setup_ui(&mut commands, ui_camera, &graph.0);
    }

    fn setup_cameras(commands: &mut Commands) -> Entity {
        commands.spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.018, 0.020, 0.027)),
                ..default()
            },
            Tonemapping::TonyMcMapface,
            Bloom::NATURAL,
            Transform::from_xyz(0.0, 2.2, 5.6).looking_at(Vec3::ZERO, Vec3::Y),
            GraphCamera,
            OrbitCamera {
                target: Vec3::ZERO,
                radius: 5.8,
                yaw: 0.0,
                pitch: -0.35,
            },
        ));

        commands
            .spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    output_mode: CameraOutputMode::Write {
                        blend_state: Some(BlendState::ALPHA_BLENDING),
                        clear_color: ClearColorConfig::None,
                    },
                    ..default()
                },
                IsDefaultUiCamera,
            ))
            .id()
    }

    fn setup_graph(
        commands: &mut Commands,
        graph: &Graph,
        style: &GraphStyle,
        assets: &GraphAssets,
    ) {
        let root = commands
            .spawn((Transform::default(), Visibility::default(), GraphRoot))
            .id();

        commands.entity(root).with_children(|parent| {
            for node in graph.nodes() {
                let position = Vec3::from_array(node.position);
                parent.spawn((
                    Mesh3d(assets.node_mesh.clone()),
                    MeshMaterial3d(assets.node_material.clone()),
                    Transform::from_translation(position),
                    GraphNodeHandle { index: node.id },
                    Velocity::default(),
                ));
            }

            for edge in graph.edges() {
                let Some(source) = graph.node_by_id(edge.source).map(|node| node.position) else {
                    warn!("skipping edge with missing source node: {}", edge.source);
                    continue;
                };
                let Some(target) = graph.node_by_id(edge.target).map(|node| node.position) else {
                    warn!("skipping edge with missing target node: {}", edge.target);
                    continue;
                };
                parent.spawn((
                    Mesh3d(assets.edge_mesh.clone()),
                    MeshMaterial3d(assets.edge_material.clone()),
                    edge_transform(
                        Vec3::from_array(source),
                        Vec3::from_array(target),
                        style.edge_width,
                    ),
                    GraphEdgeHandle {
                        source: edge.source,
                        target: edge.target,
                    },
                ));
            }
        });
    }

    fn create_graph_assets(
        style: &GraphStyle,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> GraphAssets {
        GraphAssets {
            node_mesh: meshes.add(Sphere::new(NODE_RADIUS).mesh().ico(4).unwrap()),
            edge_mesh: meshes.add(Cylinder::new(EDGE_RADIUS, 1.0).mesh().resolution(12)),
            node_material: materials.add(emissive_material(
                Color::Srgba(style.node_color),
                emissive_from_srgba(style.node_color, style.node_glow),
            )),
            edge_material: materials.add(emissive_material(
                Color::Srgba(style.edge_color),
                emissive_from_srgba(style.edge_color, style.edge_glow),
            )),
        }
    }

    fn emissive_material(base_color: Color, emissive: LinearRgba) -> StandardMaterial {
        StandardMaterial {
            base_color,
            emissive,
            alpha_mode: if base_color.to_srgba().alpha < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            perceptual_roughness: 0.72,
            metallic: 0.0,
            ..default()
        }
    }

    fn emissive_from_srgba(color: Srgba, intensity: f32) -> LinearRgba {
        let linear = LinearRgba::from(Color::Srgba(color));
        LinearRgba::rgb(
            linear.red * intensity,
            linear.green * intensity,
            linear.blue * intensity,
        )
    }

    fn setup_ui(commands: &mut Commands, ui_camera: Entity, graph: &Graph) {
        commands.spawn(ui_root(ui_camera)).with_children(|root| {
            root.spawn(side_panel()).with_children(|panel| {
                panel.spawn(panel_header());
                spawn_graph_selector(panel);
                panel.spawn(graph_summary(graph.nodes().len(), graph.edges().len()));
                spawn_control_sections(panel);
                panel.spawn(action_button("Reset layout", ControlAction::ResetLayout));
            });

            root.spawn(graph_pane()).with_children(|pane| {
                pane.spawn(viewport_badge());
            });
        });
    }

    fn ui_root(ui_camera: Entity) -> impl Bundle {
        (
            UiTargetCamera(ui_camera),
            UiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                ..default()
            },
        )
    }

    fn side_panel() -> impl Bundle {
        (
            SidePanel,
            Node {
                width: px(PANEL_WIDTH),
                height: percent(100),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                padding: UiRect::all(px(14)),
                border: UiRect::right(px(1)),
                overflow: Overflow::scroll_y(),
                scrollbar_width: 6.0,
                ..default()
            },
            ScrollPosition::default(),
            BackgroundColor(panel_color()),
            BorderColor::all(Color::srgba(0.16, 0.18, 0.22, 1.0)),
        )
    }

    fn graph_pane() -> impl Bundle {
        (
            GraphViewportPane,
            Node {
                flex_grow: 1.0,
                height: percent(100),
                position_type: PositionType::Relative,
                ..default()
            },
        )
    }

    fn viewport_badge() -> impl Bundle {
        (
            Node {
                position_type: PositionType::Absolute,
                top: px(18),
                right: px(20),
                padding: UiRect::axes(px(10), px(7)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.025, 0.032, 0.58)),
            children![label("3D viewport", 13.0, Color::srgb(0.72, 0.78, 0.86),)],
        )
    }

    fn panel_header() -> impl Bundle {
        (
            Node {
                padding: UiRect::bottom(px(2)),
                ..column(3.0)
            },
            children![
                label("Graph Layout", 22.0, Color::WHITE),
                label("Force-directed controls", 12.0, subtle_text_color()),
            ],
        )
    }

    fn spawn_graph_selector(parent: &mut ChildSpawnerCommands) {
        parent.spawn(column(6.0)).with_children(|selector| {
            selector.spawn(section_label("Graph"));
            selector.spawn(preset_button());
            selector.spawn(preset_dropdown()).with_children(|list| {
                for preset in GraphPreset::ALL {
                    list.spawn(preset_option(preset));
                }
            });
        });
    }

    fn preset_button() -> impl Bundle {
        (
            Button,
            ControlAction::TogglePresetMenu,
            Node {
                width: percent(100),
                height: px(32),
                display: Display::Flex,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(10)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BorderColor::all(accent_border_color()),
            BackgroundColor(button_color()),
            children![
                (
                    Text::new(GraphPreset::Sample.label()),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::WHITE),
                    PresetLabel,
                ),
                label("v", 12.0, subtle_text_color()),
            ],
        )
    }

    fn preset_dropdown() -> impl Bundle {
        (
            PresetDropdownList,
            Node {
                display: Display::None,
                padding: UiRect::all(px(4)),
                border: UiRect::all(px(1)),
                ..column(2.0)
            },
            BackgroundColor(Color::srgba(0.045, 0.050, 0.062, 1.0)),
            BorderColor::all(border_color()),
        )
    }

    fn preset_option(preset: GraphPreset) -> impl Bundle {
        (
            Button,
            ControlAction::SelectPreset(preset),
            Node {
                width: percent(100),
                height: px(28),
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(8)),
                ..default()
            },
            BackgroundColor(button_color()),
            children![label(preset.label(), 13.0, main_text_color())],
        )
    }

    fn graph_summary(node_count: usize, edge_count: usize) -> impl Bundle {
        (
            row(8.0),
            children![
                stat_tile("Nodes", node_count.to_string(), GraphStat::Nodes),
                stat_tile("Edges", edge_count.to_string(), GraphStat::Edges),
            ],
        )
    }

    fn stat_tile(label_text: &'static str, value: String, stat: GraphStat) -> impl Bundle {
        (
            Node {
                flex_grow: 1.0,
                height: px(46),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(px(10)),
                border: UiRect::all(px(1)),
                ..column_with_width(Val::Auto, 2.0)
            },
            BackgroundColor(surface_color()),
            BorderColor::all(border_color()),
            children![
                label(label_text, 11.0, subtle_text_color()),
                (
                    Text::new(value),
                    TextFont::from_font_size(18.0),
                    TextColor(Color::WHITE),
                    stat,
                ),
            ],
        )
    }

    fn spawn_control_sections(parent: &mut ChildSpawnerCommands) {
        for section in CONTROL_SECTIONS {
            spawn_control_section(parent, *section);
        }
    }

    fn spawn_control_section(parent: &mut ChildSpawnerCommands, section: ControlSection) {
        parent
            .spawn(control_card(section.gap))
            .with_children(|card| {
                if !section.title.is_empty() {
                    card.spawn(section_label(section.title));
                }
                for row in section.rows {
                    spawn_control_row(card, *row);
                }
            });
    }

    fn spawn_control_row(parent: &mut ChildSpawnerCommands, row: ControlRow) {
        match row {
            ControlRow::Scalar(text, target) => {
                parent.spawn(scalar_row(text, target));
            }
            ControlRow::Toggle(text, readout, action) => {
                parent.spawn(toggle_row(text, readout, action));
            }
            ControlRow::NodeColorHeader => {
                parent.spawn(spaced_row(children![
                    label("Node color", 11.0, subtle_text_color()),
                    node_color_swatch(),
                ]));
            }
            ControlRow::ColorPlane => {
                parent.spawn(node_color_plane());
            }
            ControlRow::ColorSlider(text, channel) => {
                parent.spawn(color_slider_row(text, channel));
            }
        }
    }

    fn control_card(row_gap: f32) -> impl Bundle {
        (
            Node {
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                ..column(row_gap)
            },
            BackgroundColor(surface_color()),
            BorderColor::all(border_color()),
        )
    }

    fn section_label(text: &'static str) -> impl Bundle {
        (
            Node {
                width: percent(100),
                padding: UiRect::bottom(px(2)),
                ..default()
            },
            children![label(text, 11.0, subtle_text_color())],
        )
    }

    fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (
            Text::new(text),
            TextFont::from_font_size(size),
            TextColor(color),
        )
    }

    fn column(row_gap: f32) -> Node {
        column_with_width(percent(100), row_gap)
    }

    fn column_with_width(width: Val, row_gap: f32) -> Node {
        Node {
            width,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(row_gap),
            ..default()
        }
    }

    fn row(column_gap: f32) -> Node {
        Node {
            width: percent(100),
            display: Display::Flex,
            align_items: AlignItems::Center,
            column_gap: px(column_gap),
            ..default()
        }
    }

    fn spaced_row_node() -> Node {
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.0)
        }
    }

    fn centered_node(width: Val, height: Val) -> Node {
        Node {
            width,
            height,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        }
    }

    fn track_node(top: f32, height: f32) -> Node {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(top),
            height: px(height),
            border_radius: BorderRadius::MAX,
            ..default()
        }
    }

    fn scalar_row(label_text: &'static str, target: ScalarTarget) -> impl Bundle {
        (
            column(5.0),
            children![
                spaced_row(children![
                    label(label_text, 13.0, main_text_color()),
                    scalar_readout(target),
                ]),
                scalar_slider(target, px(24)),
            ],
        )
    }

    fn toggle_row(
        label_text: &'static str,
        readout: StatusReadout,
        action: ControlAction,
    ) -> impl Bundle {
        spaced_row(children![
            label(label_text, 13.0, main_text_color()),
            (
                Button,
                action,
                Node {
                    border: UiRect::all(px(1)),
                    ..centered_node(px(82), px(28))
                },
                BorderColor::all(accent_border_color()),
                BackgroundColor(button_color()),
                children![status_readout(readout)],
            ),
        ])
    }

    fn spaced_row(children: impl Bundle) -> impl Bundle {
        (spaced_row_node(), children)
    }

    fn scalar_readout(target: ScalarTarget) -> impl Bundle {
        (
            Text::new(""),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.72, 0.82, 0.88)),
            ScalarReadout(target),
        )
    }

    fn status_readout(readout: StatusReadout) -> impl Bundle {
        (
            Text::new(""),
            TextFont::from_font_size(13.0),
            TextColor(Color::srgb(0.72, 0.82, 0.88)),
            readout,
        )
    }

    fn scalar_slider(target: ScalarTarget, height: Val) -> impl Bundle {
        let spec = target.spec();
        let (min, max) = spec.range;
        (
            GraphSlider,
            ScalarControl(target),
            Slider {
                track_click: TrackClick::Snap,
                orientation: SliderOrientation::Horizontal,
            },
            SliderValue(spec.default_value),
            SliderRange::new(min, max),
            SliderStep(spec.step),
            SliderPrecision(spec.precision),
            Node {
                width: percent(100),
                height,
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                slider_track(Color::srgba(0.035, 0.040, 0.052, 1.0)),
                slider_fill(spec.fill_color),
                slider_thumb(),
            ],
        )
    }

    fn slider_track(color: Color) -> impl Bundle {
        (track_node(10.0, 5.0), BackgroundColor(color))
    }

    fn slider_fill(color: Color) -> impl Bundle {
        (
            GraphSliderFill,
            Node {
                width: percent(0),
                right: Val::Auto,
                ..track_node(10.0, 5.0)
            },
            BackgroundColor(color),
        )
    }

    fn slider_thumb() -> impl Bundle {
        (
            GraphSliderThumb,
            SliderThumb,
            Node {
                position_type: PositionType::Absolute,
                left: percent(0),
                top: percent(50),
                width: px(12),
                height: px(18),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BorderColor::all(Color::srgba(0.82, 0.86, 0.90, 1.0)),
            BackgroundColor(Color::srgba(0.17, 0.19, 0.23, 1.0)),
            UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
            Pickable::IGNORE,
        )
    }

    fn node_color_plane() -> impl Bundle {
        (
            NodeColorPlane,
            Node {
                width: percent(100),
                height: px(96),
                padding: UiRect::all(px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.040, 0.052, 1.0)),
            BorderColor::all(border_color()),
            children![(
                NodeColorPlaneInner,
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Relative,
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                color_plane_gradient(GraphStyle::default().node_color),
                children![(
                    NodeColorPlaneThumb,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(0),
                        top: percent(0),
                        width: px(11),
                        height: px(11),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::WHITE),
                    BackgroundColor(Color::srgba(0.02, 0.025, 0.032, 1.0)),
                    UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                    Pickable::IGNORE,
                )],
            )],
        )
    }

    fn node_color_swatch() -> impl Bundle {
        (
            NodeColorSwatch,
            Node {
                width: px(26),
                height: px(22),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.70, 0.76, 0.82, 0.8)),
            BackgroundColor(Color::Srgba(GraphStyle::default().node_color)),
        )
    }

    fn color_slider_row(label_text: &'static str, channel: ColorChannel) -> impl Bundle {
        (
            row(8.0),
            children![
                (
                    centered_node(px(14), Val::Auto),
                    children![label(label_text, 12.0, subtle_text_color())],
                ),
                color_slider(channel),
            ],
        )
    }

    fn color_slider(channel: ColorChannel) -> impl Bundle {
        let target = ScalarTarget::NodeColor(channel);
        let spec = target.spec();
        let (min, max) = spec.range;
        (
            GraphSlider,
            ScalarControl(target),
            Slider {
                track_click: TrackClick::Snap,
                orientation: SliderOrientation::Horizontal,
            },
            SliderValue(spec.default_value),
            SliderRange::new(min, max),
            SliderStep(spec.step),
            SliderPrecision(spec.precision),
            Node {
                width: percent(100),
                height: px(18),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                ..default()
            },
            children![
                (
                    ColorSliderTrack(channel),
                    track_node(6.0, 6.0),
                    color_channel_gradient(channel, GraphStyle::default().node_color),
                ),
                slider_thumb(),
            ],
        )
    }

    fn color_plane_gradient(color: Srgba) -> BackgroundGradient {
        horizontal_gradient(
            Color::srgb(0.0, color.green, 0.0),
            Color::srgb(1.0, color.green, 1.0),
        )
    }

    fn color_channel_gradient(channel: ColorChannel, color: Srgba) -> BackgroundGradient {
        let (start, end) = match channel {
            ColorChannel::Red => (
                Color::srgb(0.0, color.green, color.blue),
                Color::srgb(1.0, color.green, color.blue),
            ),
            ColorChannel::Green => (
                Color::srgb(color.red, 0.0, color.blue),
                Color::srgb(color.red, 1.0, color.blue),
            ),
            ColorChannel::Blue => (
                Color::srgb(color.red, color.green, 0.0),
                Color::srgb(color.red, color.green, 1.0),
            ),
        };

        horizontal_gradient(start, end)
    }

    fn horizontal_gradient(start: Color, end: Color) -> BackgroundGradient {
        BackgroundGradient(vec![Gradient::Linear(LinearGradient {
            angle: std::f32::consts::FRAC_PI_2,
            stops: vec![
                ColorStop::new(start, percent(0)),
                ColorStop::new(end, percent(100)),
            ],
            color_space: InterpolationColorSpace::Srgba,
        })])
    }

    fn action_button(text: &'static str, action: ControlAction) -> impl Bundle {
        (
            Button,
            action,
            Node {
                border: UiRect::all(px(1)),
                ..centered_node(percent(100), px(34))
            },
            BorderColor::all(accent_border_color()),
            BackgroundColor(Color::srgba(0.10, 0.18, 0.22, 1.0)),
            children![label(text, 14.0, Color::WHITE)],
        )
    }

    fn panel_color() -> Color {
        Color::srgba(0.035, 0.040, 0.052, 0.98)
    }

    fn surface_color() -> Color {
        Color::srgba(0.062, 0.070, 0.086, 0.96)
    }

    fn button_color() -> Color {
        Color::srgba(0.10, 0.12, 0.15, 1.0)
    }

    fn border_color() -> Color {
        Color::srgba(0.19, 0.22, 0.27, 1.0)
    }

    fn accent_border_color() -> Color {
        Color::srgba(0.28, 0.48, 0.58, 1.0)
    }

    fn main_text_color() -> Color {
        Color::srgb(0.86, 0.89, 0.94)
    }

    fn subtle_text_color() -> Color {
        Color::srgb(0.56, 0.62, 0.70)
    }

    fn update_graph_camera_viewport(
        windows: Query<&Window>,
        mut cameras: Query<&mut Camera, With<GraphCamera>>,
    ) {
        let Ok(window) = windows.single() else {
            return;
        };
        let Ok(mut camera) = cameras.single_mut() else {
            return;
        };

        let window_size = window.physical_size();
        let panel_width = (PANEL_WIDTH * window.scale_factor()) as u32;
        let viewport_width = window_size.x.saturating_sub(panel_width).max(1);

        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(panel_width.min(window_size.x), 0),
            physical_size: UVec2::new(viewport_width, window_size.y.max(1)),
            ..default()
        });
    }

    fn send_scroll_events(
        mut mouse_wheel_reader: MessageReader<MouseWheel>,
        hover_map: Res<HoverMap>,
        mut commands: Commands,
    ) {
        for mouse_wheel in mouse_wheel_reader.read() {
            let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);
            if mouse_wheel.unit == MouseScrollUnit::Line {
                delta *= 21.0;
            }

            for pointer_map in hover_map.0.values() {
                for entity in pointer_map.keys().copied() {
                    commands.trigger(Scroll { entity, delta });
                }
            }
        }
    }

    fn on_scroll_handler(
        mut scroll: On<Scroll>,
        mut query: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
    ) {
        let Ok((mut scroll_position, node, computed)) = query.get_mut(scroll.entity) else {
            return;
        };

        let max_offset =
            (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
        let delta = &mut scroll.delta;

        if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 {
            let max = if delta.x > 0.0 {
                scroll_position.x >= max_offset.x
            } else {
                scroll_position.x <= 0.0
            };

            if !max {
                scroll_position.x = (scroll_position.x + delta.x).clamp(0.0, max_offset.x.max(0.0));
                delta.x = 0.0;
            }
        }

        if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 {
            let max = if delta.y > 0.0 {
                scroll_position.y >= max_offset.y
            } else {
                scroll_position.y <= 0.0
            };

            if !max {
                scroll_position.y = (scroll_position.y + delta.y).clamp(0.0, max_offset.y.max(0.0));
                delta.y = 0.0;
            }
        }

        if *delta == Vec2::ZERO {
            scroll.propagate(false);
        }
    }

    fn orbit_graph_camera(
        windows: Query<&Window>,
        mouse_buttons: Res<ButtonInput<MouseButton>>,
        mouse_motion: Res<AccumulatedMouseMotion>,
        mut mouse_wheel_reader: MessageReader<MouseWheel>,
        mut cameras: Query<(&mut Transform, &mut OrbitCamera), With<GraphCamera>>,
    ) {
        let Ok(window) = windows.single() else {
            return;
        };
        let Ok((mut transform, mut orbit)) = cameras.single_mut() else {
            return;
        };

        let cursor_in_viewport = window
            .cursor_position()
            .is_some_and(|position| position.x >= PANEL_WIDTH);

        if cursor_in_viewport && mouse_buttons.pressed(MouseButton::Right) {
            orbit.yaw -= mouse_motion.delta.x * 0.006;
            orbit.pitch = (orbit.pitch - mouse_motion.delta.y * 0.006).clamp(-1.25, 1.15);
        }

        if cursor_in_viewport {
            for wheel in mouse_wheel_reader.read() {
                let unit_scale = match wheel.unit {
                    MouseScrollUnit::Line => 0.22,
                    MouseScrollUnit::Pixel => 0.018,
                };
                orbit.radius = (orbit.radius - wheel.y * unit_scale).clamp(2.4, 12.0);
            }
        }

        let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
        transform.translation = orbit.target + rotation * Vec3::new(0.0, 0.0, orbit.radius);
        transform.look_at(orbit.target, Vec3::Y);
    }

    fn handle_control_buttons(
        mut interactions: Query<
            (&Interaction, &ControlAction),
            (Changed<Interaction>, With<Button>),
        >,
        mut preset_state: ResMut<GraphPresetState>,
        mut pending_preset: ResMut<PendingGraphPreset>,
        mut controls: ResMut<LayoutControls>,
        graph: Res<GraphResource>,
        mut nodes: Query<(&GraphNodeHandle, &mut Transform, &mut Velocity)>,
    ) {
        for (interaction, action) in &mut interactions {
            if *interaction != Interaction::Pressed {
                continue;
            }

            match *action {
                ControlAction::ToggleSimulation => {
                    controls.simulation_running = !controls.simulation_running;
                }
                ControlAction::ToggleRotation => {
                    controls.slow_rotation = !controls.slow_rotation;
                }
                ControlAction::ResetLayout => {
                    reset_layout(&graph.0, &mut nodes);
                }
                ControlAction::TogglePresetMenu => {
                    preset_state.menu_open = !preset_state.menu_open;
                }
                ControlAction::SelectPreset(preset) => {
                    preset_state.menu_open = false;

                    if preset_state.selected != preset {
                        preset_state.selected = preset;
                        pending_preset.0 = Some(preset);
                    }
                }
            }
        }
    }

    fn apply_selected_preset(
        mut commands: Commands,
        mut pending_preset: ResMut<PendingGraphPreset>,
        style: Res<GraphStyle>,
        assets: Res<GraphAssets>,
        graph_roots: Query<Entity, With<GraphRoot>>,
    ) {
        let Some(preset) = pending_preset.0.take() else {
            return;
        };

        let next_graph = preset.graph();
        for root in &graph_roots {
            commands.entity(root).despawn();
        }

        setup_graph(&mut commands, &next_graph, &style, &assets);
        commands.insert_resource(GraphResource(next_graph));
    }

    fn update_button_colors(
        mut buttons: Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<Button>),
        >,
    ) {
        for (interaction, mut color) in &mut buttons {
            color.0 = match *interaction {
                Interaction::Pressed => Color::srgba(0.18, 0.34, 0.42, 1.0),
                Interaction::Hovered => Color::srgba(0.14, 0.22, 0.28, 1.0),
                Interaction::None => button_color(),
            };
        }
    }

    fn update_graph_selector(
        preset_state: Res<GraphPresetState>,
        mut labels: Query<&mut Text, With<PresetLabel>>,
        mut dropdowns: Query<&mut Node, With<PresetDropdownList>>,
    ) {
        if !preset_state.is_changed() {
            return;
        }

        for mut text in &mut labels {
            *text = Text::new(preset_state.selected.label());
        }

        for mut node in &mut dropdowns {
            node.display = if preset_state.menu_open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    fn update_graph_readouts(graph: Res<GraphResource>, mut stats: Query<(&GraphStat, &mut Text)>) {
        if !graph.is_changed() {
            return;
        }

        for (stat, mut text) in &mut stats {
            *text = Text::new(match stat {
                GraphStat::Nodes => graph.0.nodes().len().to_string(),
                GraphStat::Edges => graph.0.edges().len().to_string(),
            });
        }
    }

    fn handle_scalar_slider_change(
        change: On<ValueChange<f32>>,
        controls_query: Query<&ScalarControl>,
        mut controls: ResMut<LayoutControls>,
        mut style: ResMut<GraphStyle>,
        mut commands: Commands,
    ) {
        let source = change.source;
        let Ok(control) = controls_query.get(source) else {
            return;
        };

        let value = match control.0 {
            ScalarTarget::Layout(field) => {
                field.set(&mut controls.settings, change.value);
                field.value(&controls.settings)
            }
            ScalarTarget::Style(field) => {
                field.set(&mut style, change.value);
                field.value(&style)
            }
            ScalarTarget::NodeColor(channel) => {
                channel.set(&mut style.node_color, change.value);
                channel.value(style.node_color)
            }
        };

        commands.entity(source).insert(SliderValue(value));
    }

    fn sync_scalar_widgets(
        controls: Res<LayoutControls>,
        style: Res<GraphStyle>,
        sliders: Query<(Entity, &ScalarControl)>,
        mut commands: Commands,
    ) {
        if !controls.is_changed() && !style.is_changed() {
            return;
        }

        for (entity, control) in &sliders {
            commands
                .entity(entity)
                .insert(SliderValue(control.0.value(&controls, &style)));
        }
    }

    fn sync_node_color_swatch(
        style: Res<GraphStyle>,
        mut swatches: Query<&mut BackgroundColor, With<NodeColorSwatch>>,
    ) {
        if !style.is_changed() {
            return;
        }

        let node_color = Color::Srgba(style.node_color);
        for mut swatch in &mut swatches {
            swatch.0 = node_color;
        }
    }

    fn update_graph_style(
        style: Res<GraphStyle>,
        assets: Res<GraphAssets>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        if !style.is_changed() {
            return;
        }

        let node_color = Color::Srgba(style.node_color);
        let node_emissive = emissive_from_srgba(style.node_color, style.node_glow);
        if let Some(mut material) = materials.get_mut(&assets.node_material) {
            set_material_style(&mut material, node_color, node_emissive, AlphaMode::Opaque);
        }

        let edge_color = Color::Srgba(style.edge_color);
        let edge_emissive = emissive_from_srgba(style.edge_color, style.edge_glow);
        if let Some(mut material) = materials.get_mut(&assets.edge_material) {
            set_material_style(&mut material, edge_color, edge_emissive, AlphaMode::Blend);
        }
    }

    fn set_material_style(
        material: &mut StandardMaterial,
        base_color: Color,
        emissive: LinearRgba,
        alpha_mode: AlphaMode,
    ) {
        material.base_color = base_color;
        material.emissive = emissive;
        material.alpha_mode = alpha_mode;
    }

    fn update_slider_visuals(
        sliders: Query<(Entity, &SliderValue, &SliderRange), With<GraphSlider>>,
        children: Query<&Children>,
        mut nodes: Query<&mut Node>,
        fills: Query<(), With<GraphSliderFill>>,
        thumbs: Query<(), With<GraphSliderThumb>>,
    ) {
        for (slider, value, range) in &sliders {
            let position = range.thumb_position(value.0).clamp(0.0, 1.0) * 100.0;
            for child in children.iter_descendants(slider) {
                if fills.contains(child) {
                    if let Ok(mut node) = nodes.get_mut(child) {
                        node.width = percent(position);
                    }
                } else if thumbs.contains(child) {
                    if let Ok(mut node) = nodes.get_mut(child) {
                        node.left = percent(position);
                    }
                }
            }
        }
    }

    fn update_color_slider_tracks(
        style: Res<GraphStyle>,
        mut tracks: Query<(&ColorSliderTrack, &mut BackgroundGradient)>,
    ) {
        if !style.is_changed() {
            return;
        }

        for (track, mut gradient) in &mut tracks {
            *gradient = color_channel_gradient(track.0, style.node_color);
        }
    }

    fn update_node_color_plane_visuals(
        style: Res<GraphStyle>,
        mut plane_gradients: Query<&mut BackgroundGradient, With<NodeColorPlaneInner>>,
        mut thumbs: Query<&mut Node, With<NodeColorPlaneThumb>>,
    ) {
        if !style.is_changed() {
            return;
        }

        for mut gradient in &mut plane_gradients {
            *gradient = color_plane_gradient(style.node_color);
        }

        for mut thumb in &mut thumbs {
            thumb.left = percent(style.node_color.red * 100.0);
            thumb.top = percent(style.node_color.blue * 100.0);
        }
    }

    fn handle_node_color_plane_press(
        mut press: On<Pointer<Press>>,
        inners: Query<
            (
                &ComputedNode,
                &ComputedUiRenderTargetInfo,
                &UiGlobalTransform,
            ),
            With<NodeColorPlaneInner>,
        >,
        ui_scale: Res<UiScale>,
        mut style: ResMut<GraphStyle>,
    ) {
        if let Ok(inner) = inners.get(press.entity) {
            press.propagate(false);
            apply_node_color_plane_change(
                &mut style,
                inner,
                press.pointer_location.position,
                ui_scale.0,
            );
        }
    }

    fn handle_node_color_plane_drag(
        mut drag: On<Pointer<Drag>>,
        inners: Query<
            (
                &ComputedNode,
                &ComputedUiRenderTargetInfo,
                &UiGlobalTransform,
            ),
            With<NodeColorPlaneInner>,
        >,
        ui_scale: Res<UiScale>,
        mut style: ResMut<GraphStyle>,
    ) {
        if let Ok(inner) = inners.get(drag.entity) {
            drag.propagate(false);
            apply_node_color_plane_change(
                &mut style,
                inner,
                drag.pointer_location.position,
                ui_scale.0,
            );
        }
    }

    fn handle_node_color_plane_drag_end(
        mut drag_end: On<Pointer<DragEnd>>,
        inners: Query<
            (
                &ComputedNode,
                &ComputedUiRenderTargetInfo,
                &UiGlobalTransform,
            ),
            With<NodeColorPlaneInner>,
        >,
        ui_scale: Res<UiScale>,
        mut style: ResMut<GraphStyle>,
    ) {
        if let Ok(inner) = inners.get(drag_end.entity) {
            drag_end.propagate(false);
            apply_node_color_plane_change(
                &mut style,
                inner,
                drag_end.pointer_location.position,
                ui_scale.0,
            );
        }
    }

    fn apply_node_color_plane_change(
        style: &mut ResMut<GraphStyle>,
        (node, target, transform): ColorPlaneInner,
        pointer_position: Vec2,
        ui_scale: f32,
    ) {
        emit_node_color_plane_change(
            &mut *style,
            node,
            target,
            transform,
            pointer_position,
            ui_scale,
        );
        style.set_changed();
    }

    fn emit_node_color_plane_change(
        style: &mut GraphStyle,
        node: &ComputedNode,
        target: &ComputedUiRenderTargetInfo,
        transform: &UiGlobalTransform,
        pointer_position: Vec2,
        ui_scale: f32,
    ) {
        let Some(position) = node.normalize_point(
            *transform,
            pointer_position * target.scale_factor() / ui_scale,
        ) else {
            return;
        };

        let value = (position + Vec2::splat(0.5)).clamp(Vec2::ZERO, Vec2::ONE);
        style.node_color.red = value.x;
        style.node_color.blue = value.y;
    }

    fn update_scalar_readouts(
        controls: Res<LayoutControls>,
        style: Res<GraphStyle>,
        mut readouts: Query<(&ScalarReadout, &mut Text)>,
    ) {
        if !controls.is_changed() && !style.is_changed() {
            return;
        }

        for (readout, mut text) in &mut readouts {
            *text = Text::new(readout.0.readout(&controls, &style));
        }
    }

    fn update_status_readouts(
        controls: Res<LayoutControls>,
        mut readouts: Query<(&StatusReadout, &mut Text)>,
    ) {
        if !controls.is_changed() {
            return;
        }

        for (readout, mut text) in &mut readouts {
            *text = Text::new(match readout {
                StatusReadout::Simulation => {
                    if controls.simulation_running {
                        "On"
                    } else {
                        "Off"
                    }
                }
                StatusReadout::Rotation => {
                    if controls.slow_rotation {
                        "On"
                    } else {
                        "Off"
                    }
                }
            });
        }
    }

    fn apply_force_layout(
        time: Res<Time>,
        graph: Res<GraphResource>,
        controls: Res<LayoutControls>,
        mut nodes: Query<(&GraphNodeHandle, &mut Transform, &mut Velocity)>,
    ) {
        if !controls.simulation_running {
            return;
        }

        let dt = time.delta_secs().min(1.0 / 30.0);
        let mut positions = Vec::new();

        for (handle, transform, velocity) in &nodes {
            positions.push((handle.index, transform.translation, velocity.0));
        }

        let mut forces = vec![Vec3::ZERO; positions.len()];

        for a in 0..positions.len() {
            for b in (a + 1)..positions.len() {
                let offset = positions[a].1 - positions[b].1;
                let distance_squared = offset.length_squared().max(0.025);
                let direction = offset.normalize_or_zero();
                let force = direction * controls.settings.repulsion / distance_squared;
                forces[a] += force;
                forces[b] -= force;
            }
        }

        for edge in graph.0.edges() {
            let Some(source) = positions
                .iter()
                .position(|(index, _, _)| *index == edge.source)
            else {
                continue;
            };
            let Some(target) = positions
                .iter()
                .position(|(index, _, _)| *index == edge.target)
            else {
                continue;
            };

            let offset = positions[target].1 - positions[source].1;
            let distance = offset.length().max(0.001);
            let stretch = distance - controls.settings.target_edge_length;
            let force =
                offset.normalize_or_zero() * stretch * controls.settings.attraction * edge.strength;
            forces[source] += force;
            forces[target] -= force;
        }

        for (handle, mut transform, mut velocity) in &mut nodes {
            let Some(slot) = positions
                .iter()
                .position(|(index, _, _)| *index == handle.index)
            else {
                continue;
            };

            let Some(node) = graph.0.node_by_id(handle.index) else {
                continue;
            };
            let mass = node.mass.max(0.1);
            velocity.0 = (velocity.0 + forces[slot] / mass * dt) * controls.settings.damping;
            velocity.0 = velocity.0.clamp_length_max(3.0);
            transform.translation += velocity.0 * dt;
        }
    }

    fn sync_edge_transforms(
        style: Res<GraphStyle>,
        nodes: Query<(&GraphNodeHandle, &Transform), Without<GraphEdgeHandle>>,
        mut edges: Query<(&GraphEdgeHandle, &mut Transform)>,
    ) {
        let positions = nodes
            .iter()
            .map(|(handle, transform)| (handle.index, transform.translation))
            .collect::<Vec<_>>();

        for (edge, mut transform) in &mut edges {
            let Some(source) = positions
                .iter()
                .find_map(|(index, position)| (*index == edge.source).then_some(*position))
            else {
                continue;
            };
            let Some(target) = positions
                .iter()
                .find_map(|(index, position)| (*index == edge.target).then_some(*position))
            else {
                continue;
            };

            *transform = edge_transform(source, target, style.edge_width);
        }
    }

    fn rotate_graph_slowly(
        time: Res<Time>,
        controls: Res<LayoutControls>,
        mut graph_roots: Query<&mut Transform, With<GraphRoot>>,
    ) {
        if !controls.slow_rotation {
            return;
        }

        for mut transform in &mut graph_roots {
            transform.rotate_y(time.delta_secs() * 0.18);
        }
    }

    fn reset_layout(
        graph: &Graph,
        nodes: &mut Query<(&GraphNodeHandle, &mut Transform, &mut Velocity)>,
    ) {
        for (handle, mut transform, mut velocity) in nodes {
            if let Some(node) = graph.node_by_id(handle.index) {
                transform.translation = Vec3::from_array(node.position);
                velocity.0 = Vec3::ZERO;
            }
        }
    }

    #[cfg(all(test, feature = "bevy-render"))]
    mod render_tests {
        use super::*;

        #[test]
        fn force_layout_ignores_stale_node_entities_after_preset_change() {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            app.insert_resource(GraphResource(GraphPreset::Single.graph()));
            app.insert_resource(LayoutControls::default());
            app.world_mut().spawn((
                GraphNodeHandle { index: 1 },
                Transform::default(),
                Velocity::default(),
            ));
            app.add_systems(Update, apply_force_layout);

            app.update();
        }
    }

    fn edge_transform(source: Vec3, target: Vec3, edge_width: f32) -> Transform {
        let offset = target - source;
        let length = offset.length().max(0.001);
        let midpoint = source + offset * 0.5;
        let rotation = Quat::from_rotation_arc(Vec3::Y, offset.normalize_or_zero());

        Transform {
            translation: midpoint,
            rotation,
            scale: Vec3::new(edge_width, length, edge_width),
        }
    }

    fn log_startup_visual_state(
        mut diagnostics: ResMut<StartupDiagnostics>,
        windows: Query<&Window>,
        graph_camera: Query<(&Camera, &GlobalTransform, &OrbitCamera), With<GraphCamera>>,
        ui_camera: Query<(Entity, &Camera), (With<Camera2d>, With<IsDefaultUiCamera>)>,
        ui_root: Query<
            (
                &ComputedNode,
                Option<&ComputedUiTargetCamera>,
                Option<&ComputedUiRenderTargetInfo>,
            ),
            With<UiRoot>,
        >,
        side_panel: Query<&ComputedNode, With<SidePanel>>,
        graph_pane: Query<&ComputedNode, With<GraphViewportPane>>,
        nodes: Query<(&GraphNodeHandle, &GlobalTransform)>,
        edges: Query<&GraphEdgeHandle>,
    ) {
        if diagnostics.printed {
            return;
        }

        diagnostics.frames_seen += 1;
        if diagnostics.frames_seen < 8 {
            return;
        }
        diagnostics.printed = true;

        macro_rules! single_line {
            ($query:expr, $unavailable:literal, |$value:pat_param| $line:expr) => {
                $query
                    .single()
                    .map_or_else(|_| $unavailable.to_string(), |$value| $line)
            };
        }

        let window_line = single_line!(windows, "window: unavailable", |window| {
            format!(
                "window: logical={:.0}x{:.0} physical={:?} scale={:.2}",
                window.width(),
                window.height(),
                window.physical_size(),
                window.scale_factor()
            )
        });

        let graph_camera_line = single_line!(
            graph_camera,
            "graph camera: unavailable",
            |(camera, transform, orbit)| {
                format!(
                    "graph camera: active={} order={} viewport={} position={:?} orbit_radius={:.2}",
                    camera.is_active,
                    camera.order,
                    format_viewport(camera.viewport.as_ref()),
                    transform.translation(),
                    orbit.radius
                )
            }
        );

        let ui_camera_line =
            single_line!(ui_camera, "ui camera: unavailable", |(entity, camera)| {
                format!(
                    "ui camera: entity={entity:?} active={} order={} viewport={} output_mode={}",
                    camera.is_active,
                    camera.order,
                    format_viewport(camera.viewport.as_ref()),
                    format_camera_output_mode(camera.output_mode)
                )
            });

        let ui_root_line = single_line!(ui_root, "ui root: unavailable", |(
            node,
            target_camera,
            target_info,
        )| {
            format!(
                "ui root: size_px={:?} logical={:?} target_camera={:?} target_physical={:?}",
                node.size(),
                logical_node_size(node),
                target_camera.and_then(ComputedUiTargetCamera::get),
                target_info.map(ComputedUiRenderTargetInfo::physical_size)
            )
        });

        let panel_line = single_line!(side_panel, "side panel: unavailable", |node| {
            format!(
                "side panel: size_px={:?} logical={:?} expected_logical_width={PANEL_WIDTH:.0}",
                node.size(),
                logical_node_size(node)
            )
        });

        let graph_pane_line = single_line!(graph_pane, "graph pane: unavailable", |node| {
            format!(
                "graph pane: size_px={:?} logical={:?}",
                node.size(),
                logical_node_size(node)
            )
        });

        let (node_count, bounds) =
            graph_bounds(nodes.iter().map(|(_, transform)| transform.translation()));
        let graph_line = match bounds {
            Some((min, max)) => format!(
                "graph entities: nodes={} edges={} world_bounds_min={:?} world_bounds_max={:?}",
                node_count,
                edges.iter().count(),
                min,
                max
            ),
            None => format!("graph entities: nodes=0 edges={}", edges.iter().count()),
        };

        info!(
            "visual startup state\n  {window_line}\n  {graph_camera_line}\n  {ui_camera_line}\n  {ui_root_line}\n  {panel_line}\n  {graph_pane_line}\n  {graph_line}"
        );
    }

    fn logical_node_size(node: &ComputedNode) -> Vec2 {
        node.size() * node.inverse_scale_factor
    }

    fn format_viewport(viewport: Option<&Viewport>) -> String {
        viewport.map_or_else(
            || "full target".to_string(),
            |viewport| {
                format!(
                    "pos={:?} size={:?}",
                    viewport.physical_position, viewport.physical_size
                )
            },
        )
    }

    fn format_camera_output_mode(output_mode: CameraOutputMode) -> &'static str {
        match output_mode {
            CameraOutputMode::Write {
                blend_state: Some(_),
                clear_color: ClearColorConfig::None,
            } => "write blend clear-none",
            CameraOutputMode::Write {
                blend_state: Some(_),
                clear_color: _,
            } => "write blend clear",
            CameraOutputMode::Write {
                blend_state: None,
                clear_color: ClearColorConfig::None,
            } => "write replace clear-none",
            CameraOutputMode::Write {
                blend_state: None,
                clear_color: _,
            } => "write replace clear",
            CameraOutputMode::Skip => "skip",
        }
    }

    fn graph_bounds(positions: impl Iterator<Item = Vec3>) -> (usize, Option<(Vec3, Vec3)>) {
        let mut count = 0;
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for position in positions {
            count += 1;
            min = min.min(position);
            max = max.max(position);
        }

        if count == 0 {
            (count, None)
        } else {
            (count, Some((min, max)))
        }
    }
}

#[cfg(feature = "bevy-render")]
pub fn run() {
    render_app::run();
}

#[cfg(not(feature = "bevy-render"))]
pub fn run() {
    panic!("my-graph::run requires the bevy-render feature");
}
