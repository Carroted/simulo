use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::window::PresentMode;
use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
    sprite::MaterialMesh2dBundle,
    window::PrimaryWindow,
};
use bevy_pancam::{PanCam, PanCamPlugin};
use bevy_prototype_lyon::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_rapier2d::rapier::dynamics::{RigidBodyHandle, RigidBodySet};
use rand;

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct MultiBodySpring {
    body_b_rb: RigidBodyHandle,
    local_anchor_a: Vec2,
    local_anchor_b: Vec2,
    stiffness: f32,
    damping: f32,
    target_len: f32,
}

#[derive(Component)]
struct WorldSpring {
    local_anchor_a: Vec2,
    world_anchor_b: Vec2,
    stiffness: f32,
    damping: f32,
    target_len: f32,
}

fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .insert_resource(ClearColor(Color::rgb(32. / 255., 28. / 255., 71. / 255.)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resizable: true,
                title: "New New Simulo Alpha 2".to_string(),
                mode: bevy::window::WindowMode::Windowed,
                present_mode: PresentMode::Immediate,
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ShapePlugin)
        .add_plugins(PanCamPlugin::default())
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(12.0))
        .add_systems(PostUpdate.intern(), simulate_springs)
        //.add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_input)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Camera2dBundle {
                camera: Camera { ..default() },
                ..default()
            },
            MainCamera,
        ))
        .insert(PanCam {
            grab_buttons: vec![MouseButton::Right, MouseButton::Middle],
            ..Default::default()
        });

    /* Create the ground. */
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.67, 0.58, 0.99),
                custom_size: Some(Vec2::new(10000.0, 1000.0)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., -1000., 0.)),
            ..default()
        },
        Collider::cuboid(5000.0, 500.0),
    ));

    // 1000 rigidbody boxes stacked on Y axis
    for i in 0..50 {
        commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgb(0.25, 0.25, 0.75),
                    custom_size: Some(Vec2::new(4., 4.)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(140., 1. + i as f32 * 4.1, 0.00)),
                ..default()
            },
            Collider::cuboid(2.0, 2.0),
            RigidBody::Dynamic,
        ));
    }
}

fn keyboard_input(
    mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    rapier_context: Res<RapierContext>,
    mut gizmos: Gizmos,
    mut camera_query: Query<(
        &MainCamera,
        &mut OrthographicProjection,
        &mut Transform,
        &GlobalTransform,
        &Camera,
        Without<Player>,
    )>,
    buttons: Res<Input<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    // There is only one primary window, so we can similarly get it from the query:
    let window = q_window.single();

    // get camera
    let mut camera: (
        &MainCamera,
        Mut<'_, OrthographicProjection>,
        Mut<'_, Transform>,
        &GlobalTransform,
        &Camera,
        _,
    ) = camera_query.single_mut();

    // check if the cursor is inside the window and get its position
    // then, ask bevy to convert into world coordinates, and truncate to discard Z
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.4.viewport_to_world(camera.3, cursor))
        .map(|ray| ray.origin.truncate())
    {
        if buttons.pressed(MouseButton::Left) {
            // Left button was pressed, lets spawn cube at mouse
            /*commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgb(0.25, 0.25, 0.75),
                        custom_size: Some(Vec2::new(4., 4.)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        world_position.x,
                        world_position.y,
                        0.00,
                    )),
                    ..default()
                },
                Collider::cuboid(2.0, 2.0),
                RigidBody::Dynamic,
            ));*/
            let solid = true;
            let filter = QueryFilter::default();
            if let Some((entity, projection)) =
                rapier_context.project_point(world_position, solid, filter)
            {
                // The collider closest to the point has this `handle`.
                println!(
                    "Projected point on entity {:?}. Point projection: {}",
                    entity, projection.point
                );
                println!(
                    "Point was inside of the collider shape: {}",
                    projection.is_inside
                );
            }
        }
    }
}

fn simulate_springs(
    // query all spring component and their rigidbody. each spring should have this on each side
    mut multibody_spring_query: Query<(&mut MultiBodySpring, &ExternalImpulse)>,
    mut world_spring_query: Query<(&mut WorldSpring, &ExternalImpulse)>,
    mut other_impulse_query: Query<(&mut ExternalImpulse, Without<MultiBodySpring>)>,
    // commands omg
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
) {
    // iterate over all springs
    for (mut spring, rigidbody_a_impulse) in multibody_spring_query.iter_mut() {
        // get the other impulser of the spring
        let entity = rapier_context.rigid_body_entity(spring.body_b_rb).unwrap();
        let mut rigidbody_b_stuff = other_impulse_query.get_mut(entity).unwrap();
        let rigidbody_b_impulse = rigidbody_b_stuff.0.as_mut();
        {
            // Apply an impulse of (10, 10) to the rigidbody
            rigidbody_b_impulse.impulse.x = 10.;
            rigidbody_b_impulse.impulse.y = 10.;
        }
    }

    // world ones
    for (mut spring, rigidbody_impulse) in world_spring_query.iter_mut() {
        // ok
        let point_a_world = joe;
    }
}
