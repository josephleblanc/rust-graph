#[cfg(feature = "bevy-render")]
use bevy::{
    camera::{CameraOutputMode, Viewport},
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel},
    post_process::bloom::Bloom,
    prelude::*,
    render::render_resource::BlendState,
    ui::{ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, UiSystems},
};
#[cfg(feature = "bevy-render")]
use my_graph_core::{ForceLayoutSettings, Graph};

#[cfg(feature = "bevy-render")]
const PANEL_WIDTH: f32 = 320.0;
#[cfg(feature = "bevy-render")]
const NODE_RADIUS: f32 = 0.16;
#[cfg(feature = "bevy-render")]
const EDGE_RADIUS: f32 = 0.018;

#[cfg(feature = "bevy-render")]
pub fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.015, 0.016, 0.020)))
        .insert_resource(GraphResource(Graph::sample()))
        .insert_resource(LayoutControls::default())
        .insert_resource(StartupDiagnostics::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "My Graph".to_string(),
                canvas: Some("#bevy".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                update_graph_camera_viewport,
                orbit_graph_camera,
                handle_control_buttons,
                update_button_colors,
                update_layout_readouts,
                apply_force_layout,
                sync_edge_transforms.after(apply_force_layout),
                rotate_graph_slowly,
            ),
        )
        .add_systems(
            PostUpdate,
            log_startup_visual_state.after(UiSystems::PostLayout),
        )
        .run();
}

#[cfg(not(feature = "bevy-render"))]
pub fn run() {
    panic!("my-graph::run requires the bevy-render feature");
}

#[cfg(feature = "bevy-render")]
#[derive(Resource)]
struct GraphResource(Graph);

#[cfg(feature = "bevy-render")]
#[derive(Resource)]
struct LayoutControls {
    settings: ForceLayoutSettings,
    simulation_running: bool,
    slow_rotation: bool,
}

#[cfg(feature = "bevy-render")]
impl Default for LayoutControls {
    fn default() -> Self {
        Self {
            settings: ForceLayoutSettings::default(),
            simulation_running: true,
            slow_rotation: true,
        }
    }
}

#[cfg(feature = "bevy-render")]
#[derive(Resource, Default)]
struct StartupDiagnostics {
    frames_seen: u32,
    printed: bool,
}

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct GraphCamera;

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct OrbitCamera {
    target: Vec3,
    radius: f32,
    yaw: f32,
    pitch: f32,
}

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct GraphRoot;

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct UiRoot;

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct SidePanel;

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct GraphViewportPane;

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct GraphNodeHandle {
    index: usize,
}

#[cfg(feature = "bevy-render")]
#[derive(Component)]
struct GraphEdgeHandle {
    source: usize,
    target: usize,
}

#[cfg(feature = "bevy-render")]
#[derive(Component, Default)]
struct Velocity(Vec3);

#[cfg(feature = "bevy-render")]
#[derive(Component, Clone, Copy)]
enum ControlAction {
    ToggleSimulation,
    ToggleRotation,
    ResetLayout,
    Repulsion(f32),
    Attraction(f32),
    EdgeLength(f32),
    Damping(f32),
}

#[cfg(feature = "bevy-render")]
#[derive(Component, Clone, Copy)]
enum Readout {
    Repulsion,
    Attraction,
    EdgeLength,
    Damping,
    Simulation,
    Rotation,
}

#[cfg(feature = "bevy-render")]
fn setup(
    mut commands: Commands,
    graph: Res<GraphResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ui_camera = setup_cameras(&mut commands);
    setup_graph(&mut commands, &graph.0, &mut meshes, &mut materials);
    setup_ui(&mut commands, ui_camera);
}

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
fn setup_graph(
    commands: &mut Commands,
    graph: &Graph,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let root = commands
        .spawn((Transform::default(), Visibility::default(), GraphRoot))
        .id();

    let node_mesh = meshes.add(Sphere::new(NODE_RADIUS).mesh().ico(4).unwrap());
    let edge_mesh = meshes.add(Cylinder::new(EDGE_RADIUS, 1.0).mesh().resolution(12));

    let node_materials = [
        materials.add(emissive_material(
            Color::srgb(0.36, 0.82, 0.95),
            LinearRgba::rgb(0.0, 1.8, 3.5),
        )),
        materials.add(emissive_material(
            Color::srgb(0.84, 0.67, 0.28),
            LinearRgba::rgb(2.8, 1.6, 0.2),
        )),
        materials.add(emissive_material(
            Color::srgb(0.92, 0.40, 0.53),
            LinearRgba::rgb(2.5, 0.4, 0.7),
        )),
    ];
    let edge_material = materials.add(emissive_material(
        Color::srgba(0.34, 0.45, 0.55, 0.55),
        LinearRgba::rgb(0.08, 0.18, 0.32),
    ));

    commands.entity(root).with_children(|parent| {
        for node in graph.nodes() {
            let position = Vec3::from_array(node.position);
            parent.spawn((
                Mesh3d(node_mesh.clone()),
                MeshMaterial3d(node_materials[node.id % node_materials.len()].clone()),
                Transform::from_translation(position),
                GraphNodeHandle { index: node.id },
                Velocity::default(),
            ));
        }

        for edge in graph.edges() {
            let source = graph.nodes()[edge.source].position;
            let target = graph.nodes()[edge.target].position;
            parent.spawn((
                Mesh3d(edge_mesh.clone()),
                MeshMaterial3d(edge_material.clone()),
                edge_transform(Vec3::from_array(source), Vec3::from_array(target)),
                GraphEdgeHandle {
                    source: edge.source,
                    target: edge.target,
                },
            ));
        }
    });
}

#[cfg(feature = "bevy-render")]
fn emissive_material(base_color: Color, emissive: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color,
        emissive,
        perceptual_roughness: 0.72,
        metallic: 0.0,
        ..default()
    }
}

#[cfg(feature = "bevy-render")]
fn setup_ui(commands: &mut Commands, ui_camera: Entity) {
    commands.spawn((
        UiTargetCamera(ui_camera),
        UiRoot,
        Node {
            width: percent(100),
            height: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            ..default()
        },
        children![
            (
                SidePanel,
                Node {
                    width: px(PANEL_WIDTH),
                    height: percent(100),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(14),
                    padding: UiRect::all(px(18)),
                    border: UiRect::right(px(1)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.055, 0.064, 0.082, 0.96)),
                BorderColor::all(Color::srgba(0.18, 0.22, 0.28, 1.0)),
                children![
                    label("Graph Layout", 24.0, Color::WHITE),
                    label("Force Model", 13.0, Color::srgb(0.66, 0.72, 0.80)),
                    control_row(
                        "Repulsion",
                        Readout::Repulsion,
                        ControlAction::Repulsion(-0.15),
                        ControlAction::Repulsion(0.15),
                    ),
                    control_row(
                        "Attraction",
                        Readout::Attraction,
                        ControlAction::Attraction(-0.10),
                        ControlAction::Attraction(0.10),
                    ),
                    control_row(
                        "Edge length",
                        Readout::EdgeLength,
                        ControlAction::EdgeLength(-0.10),
                        ControlAction::EdgeLength(0.10),
                    ),
                    control_row(
                        "Damping",
                        Readout::Damping,
                        ControlAction::Damping(-0.02),
                        ControlAction::Damping(0.02),
                    ),
                    section_gap(),
                    toggle_row(
                        "Simulation",
                        Readout::Simulation,
                        ControlAction::ToggleSimulation
                    ),
                    toggle_row("Orbit", Readout::Rotation, ControlAction::ToggleRotation),
                    action_button("Reset layout", ControlAction::ResetLayout),
                ],
            ),
            (
                GraphViewportPane,
                Node {
                    flex_grow: 1.0,
                    height: percent(100),
                    position_type: PositionType::Relative,
                    ..default()
                },
                children![(
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(18),
                        right: px(20),
                        padding: UiRect::axes(px(10), px(7)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.025, 0.032, 0.58)),
                    children![label("3D viewport", 13.0, Color::srgb(0.72, 0.78, 0.86),)],
                )],
            ),
        ],
    ));
}

#[cfg(feature = "bevy-render")]
fn label(text: &'static str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

#[cfg(feature = "bevy-render")]
fn control_row(
    label_text: &'static str,
    readout: Readout,
    decrease: ControlAction,
    increase: ControlAction,
) -> impl Bundle {
    (
        Node {
            width: percent(100),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        },
        children![
            (
                Node {
                    width: percent(100),
                    display: Display::Flex,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    label(label_text, 14.0, Color::srgb(0.86, 0.89, 0.94)),
                    readout_label(readout),
                ],
            ),
            (
                Node {
                    width: percent(100),
                    display: Display::Flex,
                    column_gap: px(8),
                    ..default()
                },
                children![small_button("-", decrease), small_button("+", increase)],
            ),
        ],
    )
}

#[cfg(feature = "bevy-render")]
fn toggle_row(label_text: &'static str, readout: Readout, action: ControlAction) -> impl Bundle {
    (
        Node {
            width: percent(100),
            display: Display::Flex,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            label(label_text, 14.0, Color::srgb(0.86, 0.89, 0.94)),
            (
                Button,
                action,
                Node {
                    width: px(88),
                    height: px(32),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.45, 0.58, 0.68, 1.0)),
                BackgroundColor(Color::srgba(0.12, 0.16, 0.20, 1.0)),
                children![readout_label(readout)],
            ),
        ],
    )
}

#[cfg(feature = "bevy-render")]
fn readout_label(readout: Readout) -> impl Bundle {
    (
        Text::new(""),
        TextFont::from_font_size(13.0),
        TextColor(Color::srgb(0.76, 0.84, 0.92)),
        readout,
    )
}

#[cfg(feature = "bevy-render")]
fn small_button(text: &'static str, action: ControlAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            width: px(42),
            height: px(30),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.36, 0.47, 0.58, 1.0)),
        BackgroundColor(Color::srgba(0.09, 0.12, 0.16, 1.0)),
        children![label(text, 17.0, Color::WHITE)],
    )
}

#[cfg(feature = "bevy-render")]
fn action_button(text: &'static str, action: ControlAction) -> impl Bundle {
    (
        Button,
        action,
        Node {
            width: percent(100),
            height: px(36),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            margin: UiRect::top(px(4)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.36, 0.47, 0.58, 1.0)),
        BackgroundColor(Color::srgba(0.11, 0.16, 0.20, 1.0)),
        children![label(text, 14.0, Color::WHITE)],
    )
}

#[cfg(feature = "bevy-render")]
fn section_gap() -> impl Bundle {
    (Node {
        width: percent(100),
        height: px(1),
        margin: UiRect::vertical(px(3)),
        ..default()
    },)
}

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
fn handle_control_buttons(
    mut interactions: Query<(&Interaction, &ControlAction), (Changed<Interaction>, With<Button>)>,
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
            ControlAction::Repulsion(delta) => {
                controls.settings.repulsion = (controls.settings.repulsion + delta).clamp(0.1, 8.0);
            }
            ControlAction::Attraction(delta) => {
                controls.settings.attraction =
                    (controls.settings.attraction + delta).clamp(0.05, 4.0);
            }
            ControlAction::EdgeLength(delta) => {
                controls.settings.target_edge_length =
                    (controls.settings.target_edge_length + delta).clamp(0.4, 4.0);
            }
            ControlAction::Damping(delta) => {
                controls.settings.damping = (controls.settings.damping + delta).clamp(0.65, 0.98);
            }
        }
    }
}

#[cfg(feature = "bevy-render")]
fn update_button_colors(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut color) in &mut buttons {
        color.0 = match *interaction {
            Interaction::Pressed => Color::srgba(0.20, 0.34, 0.42, 1.0),
            Interaction::Hovered => Color::srgba(0.15, 0.22, 0.28, 1.0),
            Interaction::None => Color::srgba(0.09, 0.12, 0.16, 1.0),
        };
    }
}

#[cfg(feature = "bevy-render")]
fn update_layout_readouts(
    controls: Res<LayoutControls>,
    mut readouts: Query<(&Readout, &mut Text)>,
) {
    if !controls.is_changed() {
        return;
    }

    for (readout, mut text) in &mut readouts {
        *text = Text::new(match readout {
            Readout::Repulsion => format!("{:.2}", controls.settings.repulsion),
            Readout::Attraction => format!("{:.2}", controls.settings.attraction),
            Readout::EdgeLength => format!("{:.2}", controls.settings.target_edge_length),
            Readout::Damping => format!("{:.2}", controls.settings.damping),
            Readout::Simulation => {
                if controls.simulation_running {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            Readout::Rotation => {
                if controls.slow_rotation {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
        });
    }
}

#[cfg(feature = "bevy-render")]
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

        let mass = graph.0.nodes()[handle.index].mass.max(0.1);
        velocity.0 = (velocity.0 + forces[slot] / mass * dt) * controls.settings.damping;
        velocity.0 = velocity.0.clamp_length_max(3.0);
        transform.translation += velocity.0 * dt;
    }
}

#[cfg(feature = "bevy-render")]
fn sync_edge_transforms(
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

        *transform = edge_transform(source, target);
    }
}

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
fn reset_layout(
    graph: &Graph,
    nodes: &mut Query<(&GraphNodeHandle, &mut Transform, &mut Velocity)>,
) {
    for (handle, mut transform, mut velocity) in nodes {
        if let Some(node) = graph.nodes().iter().find(|node| node.id == handle.index) {
            transform.translation = Vec3::from_array(node.position);
            velocity.0 = Vec3::ZERO;
        }
    }
}

#[cfg(feature = "bevy-render")]
fn edge_transform(source: Vec3, target: Vec3) -> Transform {
    let offset = target - source;
    let length = offset.length().max(0.001);
    let midpoint = source + offset * 0.5;
    let rotation = Quat::from_rotation_arc(Vec3::Y, offset.normalize_or_zero());

    Transform {
        translation: midpoint,
        rotation,
        scale: Vec3::new(1.0, length, 1.0),
    }
}

#[cfg(feature = "bevy-render")]
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

    let window_line = windows.single().map_or_else(
        |_| "window: unavailable".to_string(),
        |window| {
            format!(
                "window: logical={:.0}x{:.0} physical={:?} scale={:.2}",
                window.width(),
                window.height(),
                window.physical_size(),
                window.scale_factor()
            )
        },
    );

    let graph_camera_line = graph_camera.single().map_or_else(
        |_| "graph camera: unavailable".to_string(),
        |(camera, transform, orbit)| {
            format!(
                "graph camera: active={} order={} viewport={} position={:?} orbit_radius={:.2}",
                camera.is_active,
                camera.order,
                format_viewport(camera.viewport.as_ref()),
                transform.translation(),
                orbit.radius
            )
        },
    );

    let ui_camera_line = ui_camera.single().map_or_else(
        |_| "ui camera: unavailable".to_string(),
        |(entity, camera)| {
            format!(
                "ui camera: entity={entity:?} active={} order={} viewport={} output_mode={}",
                camera.is_active,
                camera.order,
                format_viewport(camera.viewport.as_ref()),
                format_camera_output_mode(camera.output_mode)
            )
        },
    );

    let ui_root_line = ui_root.single().map_or_else(
        |_| "ui root: unavailable".to_string(),
        |(node, target_camera, target_info)| {
            format!(
                "ui root: size_px={:?} logical={:?} target_camera={:?} target_physical={:?}",
                node.size(),
                logical_node_size(node),
                target_camera.and_then(ComputedUiTargetCamera::get),
                target_info.map(ComputedUiRenderTargetInfo::physical_size)
            )
        },
    );

    let panel_line = side_panel.single().map_or_else(
        |_| "side panel: unavailable".to_string(),
        |node| {
            format!(
                "side panel: size_px={:?} logical={:?} expected_logical_width={PANEL_WIDTH:.0}",
                node.size(),
                logical_node_size(node)
            )
        },
    );

    let graph_pane_line = graph_pane.single().map_or_else(
        |_| "graph pane: unavailable".to_string(),
        |node| {
            format!(
                "graph pane: size_px={:?} logical={:?}",
                node.size(),
                logical_node_size(node)
            )
        },
    );

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

#[cfg(feature = "bevy-render")]
fn logical_node_size(node: &ComputedNode) -> Vec2 {
    node.size() * node.inverse_scale_factor
}

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
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

#[cfg(feature = "bevy-render")]
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
