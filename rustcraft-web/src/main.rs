use gloo_net::http::Request;
use yew::prelude::*;
use rustcraft_lib::web::dto;
use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
#[derive(Properties, PartialEq)]
struct PlayersListProps {
    pub(crate) players: Vec<dto::Player>,
}


#[function_component(PlayersList)]
fn players_list(props: &PlayersListProps) -> Html {
    html! {
        <ul>
            { for props.players.iter().map(|player| html! {
                <li>{ &player.username }</li>
            })}
        </ul>
    }
}

#[function_component(Chat)]
fn chat() -> Html {
    let msg = use_state(|| "".to_string());
    html! {
        <div>
            <input type="text" onchange={
                let msg = msg.clone();
                Callback::from(move |e: Event| {
                    let input = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
                    if let Some(input) = input {
                        msg.set(input.value());
                    }
                })
            } />
            <button onclick={
                let msg = msg.clone();
                Callback::from(move |_| {
                    let msg = msg.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                    let result = Request::put(format!("http://localhost:8080/chat?text={}", *msg).as_str())
                        .send()
                        .await
                        .expect("could not send request");
                });
            })}> { "Send" }</button>
        </div>
    }
}

#[function_component(App)]
fn app() -> Html {
    let players = use_state(|| vec![]);
    {
        let players = players.clone();
        use_effect_with((), move |_| {
            let players = players.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let fetched_players: Vec<dto::Player> = Request::get("http://localhost:8080/players")
                    .send()
                    .await
                    .expect("could not send request")
                    .json()
                    .await
                    .expect("could not parse json");
                players.set(fetched_players);
            });
        });
    }
    html! {
    <>
        <Chat />
        <PlayersList players={ (*players).clone() } />
    </>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
