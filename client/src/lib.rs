use std::{net::UdpSocket, time::SystemTime};

use bevy::{
    DefaultPlugins,
    app::{App, FixedUpdate, PluginGroup as _, Startup, Update},
    color::Color,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        query::{Changed, With},
        schedule::IntoSystemConfigs,
        system::{Commands, Query},
    },
    hierarchy::{BuildChildren, ChildBuild, DespawnRecursiveExt},
    log,
    state::{
        app::AppExtStates,
        condition::in_state,
        state::{StateTransitionEvent, States},
    },
    text::{TextColor, TextFont},
    ui::{
        AlignItems, BackgroundColor, BorderColor, Interaction, JustifyContent, Node, UiRect, Val,
        widget::{Button, Text},
    },
    window::{Window, WindowPlugin},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierContextInitialization, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use bevy_renet::{
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::{ConnectionConfig, RenetClient},
};
use common::{
    GameLogic, GameLogicPlugin, message::ReliableMessageFromServer, net_obj::NetworkObject,
};
use network::{Instance, InstanceConnecting, ReliableMessage};
use uuid::Uuid;

pub mod network;
pub mod player;
pub mod tick;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    InGame,
}

pub fn run() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Client".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            network::NetworkPlugin,
        ))
        .add_plugins((
            GameLogicPlugin::new(in_state(AppState::InGame)),
            tick::TickPlugin,
        ))
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(player::PlayerPlugin)
        .add_plugins(
            RapierPhysicsPlugin::<NoUserData>::default()
                .with_custom_initialization(RapierContextInitialization::NoAutomaticRapierContext),
        )
        .add_plugins(RapierDebugRenderPlugin::default())
        .insert_state(AppState::MainMenu)
        .add_systems(Startup, spawn_login_screen)
        .add_systems(
            FixedUpdate,
            (
                handle_login.run_if(in_state(AppState::MainMenu)),
                despawn.in_set(GameLogic::Spawn),
            )
                .after(GameLogic::Start),
        )
        .add_systems(Update, log_state_transitions)
        .run();
}

fn log_state_transitions(mut app: EventReader<StateTransitionEvent<AppState>>) {
    for transition in app.read() {
        log::info!(
            "AppState transision: {:?} => {:?}",
            transition.exited,
            transition.entered
        );
    }
}

fn despawn(
    mut reader: EventReader<ReliableMessage>,
    mut commands: Commands,
    query: Query<(Entity, &NetworkObject)>,
) {
    for msg in reader.read() {
        let ReliableMessageFromServer::Despawn(net_obj) = &msg.message else {
            continue;
        };

        for (e, obj) in query.iter() {
            if obj == net_obj {
                commands.entity(e).despawn_recursive();
            }
        }
    }
}

#[derive(Component)]
struct LoginScreen;

fn spawn_login_screen(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..Default::default()
        },))
        .insert(LoginScreen)
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    BorderColor(Color::BLACK),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Login"),
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                        TextFont::from_font_size(40.0),
                    ));
                });
        });
}

fn handle_login(
    mut commands: Commands,
    button: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    parent: Query<Entity, With<LoginScreen>>,
) {
    for interaction in button.iter() {
        if let Interaction::Pressed = *interaction {
            commands.entity(parent.single()).despawn_recursive();

            let client = RenetClient::new(ConnectionConfig::default());
            let client_id = rand::random();
            log::info!("client_id: {client_id}");
            let authentication = ClientAuthentication::Unsecure {
                protocol_id: 0,
                client_id,
                server_addr: "127.0.0.1:6969".parse().unwrap(),
                user_data: None,
            };
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let transport =
                NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

            commands.spawn((
                network::Client::new(client, transport),
                Instance(Uuid::now_v7()),
                InstanceConnecting,
            ));
        }
    }
}
