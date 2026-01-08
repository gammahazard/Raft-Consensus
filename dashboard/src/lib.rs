//! # dashboard
//!
//! why: provide interactive web ui for raft cluster visualization
//! relations: uses shim/host.js for cluster management, displays node states
//! what: leptos components for cluster viz, chaos controls, kv store, event log

use leptos::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlInputElement;

/// main app component - raft cluster dashboard
#[component]
pub fn App() -> impl IntoView {
    // node states: 0=follower, 1=leader, 2=candidate, 3=dead
    let (node1, set_node1) = create_signal(1i32); // starts as leader
    let (node2, set_node2) = create_signal(0i32);
    let (node3, set_node3) = create_signal(0i32);
    let (term, set_term) = create_signal(1i32);
    let (events, set_events) = create_signal::<Vec<String>>(vec![]);
    let (kv_out, set_kv_out) = create_signal::<Vec<String>>(vec![]);
    
    let state_str = |s: i32| match s {
        1 => "leader",
        2 => "candidate",
        3 => "dead",
        _ => "follower",
    };
    
    let state_emoji = |s: i32| match s {
        1 => "👑",
        2 => "🗳️",
        3 => "💀",
        _ => "🟢",
    };
    
    view! {
        <div class="dashboard">
            <header class="dashboard-header">
                <h1>"🗳️ Raft Consensus Cluster"</h1>
                <span class="status-badge running">"Term: " {term}</span>
            </header>
            
            <div class="main-content">
                <div class="left-panel">
                    // cluster viz
                    <div class="card">
                        <div class="card-header"><h2>"Cluster Status"</h2></div>
                        <div class="card-body">
                            <div class="cluster-viz">
                                <div class="node" class=move || state_str(node1.get())>
                                    <div class="node-indicator">{move || state_emoji(node1.get())}</div>
                                    <div class="node-label">"Node 1"</div>
                                    <div class="node-state">{move || state_str(node1.get())}</div>
                                </div>
                                <div class="node" class=move || state_str(node2.get())>
                                    <div class="node-indicator">{move || state_emoji(node2.get())}</div>
                                    <div class="node-label">"Node 2"</div>
                                    <div class="node-state">{move || state_str(node2.get())}</div>
                                </div>
                                <div class="node" class=move || state_str(node3.get())>
                                    <div class="node-indicator">{move || state_emoji(node3.get())}</div>
                                    <div class="node-label">"Node 3"</div>
                                    <div class="node-state">{move || state_str(node3.get())}</div>
                                </div>
                            </div>
                        </div>
                    </div>
                    
                    // chaos controls
                    <div class="card">
                        <div class="card-header"><h2>"🎮 Chaos Controls"</h2></div>
                        <div class="card-body">
                            <div class="chaos-controls">
                                <button class="chaos-btn danger" on:click=move |_| {
                                    set_node1.set(3);
                                    set_events.update(|e| e.push("[CHAOS] killed node 1".into()));
                                }>"💀 Kill N1"</button>
                                <button class="chaos-btn danger" on:click=move |_| {
                                    set_node2.set(3);
                                    set_events.update(|e| e.push("[CHAOS] killed node 2".into()));
                                }>"💀 Kill N2"</button>
                                <button class="chaos-btn danger" on:click=move |_| {
                                    set_node3.set(3);
                                    set_events.update(|e| e.push("[CHAOS] killed node 3".into()));
                                }>"💀 Kill N3"</button>
                                <button class="chaos-btn success" on:click=move |_| {
                                    set_node1.set(0); set_node2.set(0); set_node3.set(0);
                                    set_events.update(|e| e.push("[CHAOS] all restarted".into()));
                                }>"🔄 Restart All"</button>
                                <button class="chaos-btn" on:click=move |_| {
                                    set_term.update(|t| *t += 1);
                                    set_node1.set(0); set_node2.set(1);
                                    set_events.update(|e| e.push("[RAFT] new leader elected".into()));
                                }>"🗳️ Election"</button>
                                <button class="chaos-btn" on:click=move |_| {
                                    set_node1.set(1); set_node2.set(0); set_node3.set(0);
                                    set_term.set(1);
                                    set_events.set(vec![]);
                                    set_kv_out.set(vec![]);
                                }>"🔄 Reset"</button>
                            </div>
                        </div>
                    </div>
                    
                    // kv store
                    <div class="card">
                        <div class="card-header"><h2>"💾 Key-Value Store"</h2></div>
                        <div class="card-body">
                            <KvStore kv_out set_kv_out node1 node2 node3 />
                        </div>
                    </div>
                </div>
                
                <div class="right-panel">
                    // metrics
                    <div class="card">
                        <div class="card-header"><h2>"📊 Metrics"</h2></div>
                        <div class="card-body">
                            <div class="metrics">
                                <div class="metric">
                                    <div class="metric-value">{move || {
                                        if node1.get() == 1 { "1" }
                                        else if node2.get() == 1 { "2" }
                                        else if node3.get() == 1 { "3" }
                                        else { "-" }
                                    }}</div>
                                    <div class="metric-label">"Leader"</div>
                                </div>
                                <div class="metric">
                                    <div class="metric-value">{term}</div>
                                    <div class="metric-label">"Term"</div>
                                </div>
                                <div class="metric">
                                    <div class="metric-value">{move || {
                                        let a = if node1.get() != 3 { 1 } else { 0 };
                                        let b = if node2.get() != 3 { 1 } else { 0 };
                                        let c = if node3.get() != 3 { 1 } else { 0 };
                                        format!("{}/3", a + b + c)
                                    }}</div>
                                    <div class="metric-label">"Alive"</div>
                                </div>
                                <div class="metric">
                                    <div class="metric-value">"~0.2"</div>
                                    <div class="metric-label">"ms"</div>
                                </div>
                            </div>
                        </div>
                    </div>
                    
                    // event log
                    <div class="card" style="flex:1">
                        <div class="card-header"><h2>"📋 Events"</h2></div>
                        <div class="card-body">
                            <div class="event-log">
                                {move || events.get().into_iter().rev().map(|e| {
                                    view! { <div class="event">{e}</div> }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// kv store component
#[component]
fn KvStore(
    kv_out: ReadSignal<Vec<String>>,
    set_kv_out: WriteSignal<Vec<String>>,
    node1: ReadSignal<i32>,
    node2: ReadSignal<i32>,
    node3: ReadSignal<i32>,
) -> impl IntoView {
    let input = create_node_ref::<leptos::html::Input>();
    
    let submit = move |_| {
        if let Some(el) = input.get() {
            let cmd = el.value();
            if cmd.is_empty() { return; }
            
            let has_leader = node1.get() == 1 || node2.get() == 1 || node3.get() == 1;
            let res = if has_leader {
                format!("> {} ✓ Committed", cmd)
            } else {
                format!("> {} ✗ No leader", cmd)
            };
            set_kv_out.update(|o| o.push(res));
            el.set_value("");
        }
    };
    
    view! {
        <div class="kv-store">
            <div class="kv-input-container">
                <input type="text" class="kv-input" placeholder="SET key value" node_ref=input />
                <button class="kv-submit" on:click=submit>"Submit"</button>
            </div>
            <div class="kv-output">
                {move || kv_out.get().into_iter().map(|l| {
                    let class = if l.contains("✓") { "success" } else { "error" };
                    view! { <div class="kv-line" class=class>{l}</div> }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// wasm entry point
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
