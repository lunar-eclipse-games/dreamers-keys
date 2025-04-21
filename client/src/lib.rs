use std::{
    collections::HashMap,
    net::UdpSocket,
    process::{Child, Command},
    time::SystemTime,
};

use bevy::{
    DefaultPlugins,
    app::{App, FixedUpdate, PluginGroup as _, Startup, Update},
    color::Color,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        query::{Changed, With, Without},
        schedule::IntoSystemConfigs,
        system::{Commands, Query, ResMut, Resource},
    },
    hierarchy::{BuildChildren, ChildBuild, DespawnRecursiveExt},
    log,
    state::{
        app::AppExtStates,
        condition::in_state,
        state::{NextState, StateTransitionEvent, States},
    },
    text::{TextColor, TextFont},
    ui::{
        AlignItems, BackgroundColor, BorderColor, FlexDirection, Interaction, JustifyContent, Node,
        UiRect, Val,
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
    netcode::{ClientAuthentication, ConnectToken, NetcodeClientTransport},
    renet::{ConnectionConfig, RenetClient},
};
use bevy_simple_text_input::{
    TextInput, TextInputInactive, TextInputPlaceholder, TextInputPlugin, TextInputSettings,
    TextInputSystem, TextInputTextColor, TextInputTextFont, TextInputValue,
};
use common::{
    GameLogic, GameLogicPlugin, instance_message,
    manager_message::{self, ReliableMessageFromClient, ReliableMessageFromServer},
    net_obj::NetworkObject,
};
use network::{
    Client, CurrentInstance, Instance, InstanceActive, InstanceConnecting, ReliableMessage,
};
use uuid::Uuid;

pub mod network;
pub mod player;
pub mod tick;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    InGame,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum LoadingState {
    Connecting,
    GetInstance,
    LoadInstance,
    Done,
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
        .add_plugins(TextInputPlugin)
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
                focus
                    .run_if(in_state(AppState::MainMenu))
                    .before(TextInputSystem),
                despawn.in_set(GameLogic::Spawn),
            )
                .after(GameLogic::Start),
        )
        .add_systems(Update, log_state_transitions)
        .insert_state(LoadingState::Connecting)
        .add_systems(
            FixedUpdate,
            (
                connect.run_if(in_state(LoadingState::Connecting)),
                receive_instance.run_if(in_state(LoadingState::GetInstance)),
                load_instance.run_if(in_state(LoadingState::LoadInstance)),
            ),
        )
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
        let instance_message::ReliableMessageFromServer::Despawn(net_obj) = &msg.message else {
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

#[derive(Component)]
struct Username;

#[derive(Component)]
struct Password;

#[derive(Component)]
struct LoginButton;

#[derive(Component)]
struct LocalButton;

fn spawn_login_screen(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..Default::default()
            },
            Interaction::None,
            LoginScreen,
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: Val::Px(200.0),
                    border: UiRect::all(Val::Px(5.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..Default::default()
                },
                BorderColor(Color::BLACK),
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                TextInput,
                TextInputTextFont(TextFont::from_font_size(30.0)),
                TextInputTextColor(TextColor(Color::srgb(0.9, 0.9, 0.9))),
                TextInputSettings {
                    retain_on_submit: true,
                    ..Default::default()
                },
                TextInputInactive(true),
                TextInputPlaceholder {
                    value: "Username".to_string(),
                    ..Default::default()
                },
                Username,
            ));

            parent.spawn((
                Node {
                    width: Val::Px(200.0),
                    border: UiRect::all(Val::Px(5.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..Default::default()
                },
                BorderColor(Color::BLACK),
                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                TextInput,
                TextInputTextFont(TextFont::from_font_size(30.0)),
                TextInputTextColor(TextColor(Color::srgb(0.9, 0.9, 0.9))),
                TextInputSettings {
                    retain_on_submit: true,
                    mask_character: Some('*'),
                    ..Default::default()
                },
                TextInputInactive(true),
                TextInputPlaceholder {
                    value: "Password".to_string(),
                    ..Default::default()
                },
                Password,
            ));

            parent
                .spawn((
                    Button,
                    LoginButton,
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
                        TextFont::from_font_size(30.0),
                    ));
                });

            parent
                .spawn((
                    Button,
                    LocalButton,
                    Node {
                        width: Val::Px(200.0),
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
                        Text::new("Local Play"),
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                        TextFont::from_font_size(30.0),
                    ));
                });
        });
}

fn focus(
    query: Query<(Entity, &Interaction), Changed<Interaction>>,
    mut text_input_query: Query<(Entity, &mut TextInputInactive)>,
) {
    for (target, interaction) in &query {
        if *interaction == Interaction::Pressed {
            for (entity, mut inactive) in text_input_query.iter_mut() {
                if entity == target {
                    inactive.0 = false;
                } else {
                    inactive.0 = true;
                }
            }
        }
    }
}

#[derive(Resource)]
struct LocalManager(Child);

impl Drop for LocalManager {
    fn drop(&mut self) {
        self.0.kill().unwrap();
    }
}

fn handle_login(
    mut commands: Commands,
    login_button: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
    local_button: Query<&Interaction, (Changed<Interaction>, With<LocalButton>)>,
    username: Query<&TextInputValue, With<Username>>,
    password: Query<&TextInputValue, With<Password>>,
    parent: Query<Entity, With<LoginScreen>>,
) {
    for interaction in login_button.iter() {
        if let Interaction::Pressed = *interaction {
            let username = username.single().0.clone();
            let password = password.single().0.clone();

            let mut map = HashMap::new();
            map.insert("user".to_owned(), username);
            map.insert("pass".to_owned(), password);

            let client = reqwest::blocking::Client::new();
            let mut token = client
                .post("http://localhost:3000/login")
                .json(&map)
                .send()
                .unwrap();

            let connect_token = ConnectToken::read(&mut token).unwrap();

            let client = RenetClient::new(ConnectionConfig::default());
            let authentication = ClientAuthentication::Secure { connect_token };
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let transport =
                NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

            commands.spawn((network::Client::new(client, transport),));

            commands.entity(parent.single()).despawn_recursive();
        }
    }

    for interaction in local_button.iter() {
        if let Interaction::Pressed = *interaction {
            #[cfg(debug_assertions)]
            let program = "./target/debug/manager";
            #[cfg(not(debug_assertions))]
            let program = "./target/release/manager";

            let process = Command::new(program).args(["local"]).spawn().unwrap();

            commands.insert_resource(LocalManager(process));

            let client = RenetClient::new(ConnectionConfig::default());
            let authentication = ClientAuthentication::Unsecure {
                protocol_id: 0,
                client_id: 0,
                server_addr: "127.0.0.1:6969".parse().unwrap(),
                user_data: None,
            };
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let transport =
                NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

            commands.spawn((network::Client::new(client, transport),));

            commands.entity(parent.single()).despawn_recursive();
        }
    }
}

fn connect(
    mut manager_client: Query<&mut Client, Without<network::Instance>>,
    mut state: ResMut<NextState<LoadingState>>,
) {
    let Ok(mut client) = manager_client.get_single_mut() else {
        return;
    };

    if client.client().is_connected() {
        client.send_manager_reliable(ReliableMessageFromClient::RequestHome);

        state.set(LoadingState::GetInstance);
    }
}

fn receive_instance(
    mut commands: Commands,
    mut state: ResMut<NextState<LoadingState>>,
    mut messages: EventReader<manager_message::ReliableMessageFromServer>,
) {
    for msg in messages.read() {
        if let ReliableMessageFromServer::Instance(data) = msg {
            log::info!("Got Instance: {}", Uuid::from_bytes(data.id));

            let connect_token = data.get_token();

            let client = RenetClient::new(ConnectionConfig::default());
            let authentication = ClientAuthentication::Secure { connect_token };
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let transport =
                NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

            commands.spawn((
                network::Client::new(client, transport),
                Instance(Uuid::from_bytes(data.id)),
                CurrentInstance,
                InstanceConnecting,
            ));

            state.set(LoadingState::LoadInstance);
        }
    }
}

fn load_instance(
    current_instance: Query<Option<&InstanceActive>, With<CurrentInstance>>,
    mut app_state: ResMut<NextState<AppState>>,
    mut loading_state: ResMut<NextState<LoadingState>>,
) {
    let current_instance = current_instance.single();

    if current_instance.is_none() {
        return;
    }

    app_state.set(AppState::InGame);
    loading_state.set(LoadingState::Done);
}
