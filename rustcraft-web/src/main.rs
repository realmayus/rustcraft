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
                <div class="card" style="width: 18rem;">
                  <div class="card-body">
                    <h5 class="card-title">{&player.username}</h5>
                    <p class="card-text">{&player.uuid}</p>
                  </div>
                  <ul class="list-group list-group-flush">
                    <li class="list-group-item">{"x-Position: "}{format!("{:.2}", &player.position.x)}</li>
                    <li class="list-group-item">{"z-Position: "}{format!("{:.2}", &player.position.z)}</li>
                    <li class="list-group-item">{"y-Position: "}{format!("{:.2}", &player.position.y)}</li>
                    <li class="list-group-item">{"Yaw: "}{format!("{:.2}", &player.position.yaw)}</li>
                    <li class="list-group-item">{"Pitch: "}{format!("{:.2}", &player.position.pitch)}</li>
                    <li class="list-group-item">{"On ground: "}{&player.position.on_ground}</li>

                  </ul>
                  <div class="card-body">
                    <a href="#" class="card-link">{"Kick"}</a>
                    <a href="#" class="card-link">{"Teleport"}</a>
                  </div>
                </div>
            })}
        </ul>
    }
}

#[function_component(Chat)]
fn chat() -> Html {
    let msg = use_state(|| "".to_string());
    html! {
        <div class="card" style="width: 20rem; padding: 20px; margin: 20px;">
                <div class="mb-3">
                    <label for="search" class="form-label">{"Send chat message"}</label>
                    <input id="search" class="form-control" type="text" onchange={
                        let msg = msg.clone();
                        Callback::from(move |e: Event| {
                            let input = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
                            if let Some(input) = input {
                                msg.set(input.value());
                            }
                        })
                    } />
                </div>
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
                    })} class="btn btn-primary"> { "Send" }</button>
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
    <div class="container-md">
        <Chat />
        <div class="card" style="width: 100%; padding: 20px; margin: 20px;">
            <PlayersList players={ (*players).clone() } />
        </div>
    </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
