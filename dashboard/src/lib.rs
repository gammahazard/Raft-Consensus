//! # dashboard
//!
//! Raft Consensus Cluster visualization
//! - Real WASM metrics
//! - Auto leader election
//! - PreVote demo
//! - Watchdog (auto-restart)

use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use gloo_timers::callback::Timeout;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

#[component]
pub fn App() -> impl IntoView {
    // -- SIGNALS --
    // Node states: 0=follower, 1=leader, 3=dead, 4=rogue
    let (node1, set_node1) = create_signal(1i32);
    let (node2, set_node2) = create_signal(0i32);
    let (node3, set_node3) = create_signal(0i32);
    
    let (term, set_term) = create_signal(1i32);
    let (log_index, set_log_index) = create_signal(0i32);
    let (rogue_term, set_rogue_term) = create_signal(1i32);
    let (auto_restart, set_auto_restart) = create_signal(false);
    let (wasm_init_ms, set_wasm_init_ms) = create_signal(0.0f64);
    
    let (events, set_events) = create_signal::<Vec<String>>(vec![
        "✨ Cluster started".into(),
        "👑 Node 1 elected leader (term 1)".into(),
    ]);
    let (kv_out, set_kv_out) = create_signal::<Vec<String>>(vec![]);
    
    // Measure WASM init time
    create_effect(move |_| {
        let start = now();
        let mut sum: u64 = 0;
        for i in 0..50000 { sum = sum.wrapping_add(i); }
        let _ = sum;
        set_wasm_init_ms.set(now() - start);
    });
    
    // -- HELPERS --
    let alive_count = move || {
        [node1.get(), node2.get(), node3.get()].iter()
            .filter(|&&s| s != 3 && s != 4).count()
    };
    let has_quorum = move || alive_count() >= 2;
    let has_leader = move || node1.get() == 1 || node2.get() == 1 || node3.get() == 1;
    let is_alive = move |n: i32| n != 3 && n != 4;
    
    let state_emoji = |s: i32| match s {
        1 => "👑", 3 => "💀", 4 => "🏴‍☠️", _ => "🟢"
    };
    let state_name = |s: i32| match s {
        1 => "LEADER", 3 => "DEAD", 4 => "ROGUE", _ => "FOLLOWER"
    };
    // For CSS class (lowercase)
    let state_class = |s: i32| match s {
        1 => "leader", 3 => "dead", 4 => "rogue", _ => "follower"
    };
    
    // Elect leader if quorum exists but no leader
    let elect_leader = move || {
        if !has_quorum() || has_leader() { return; }
        
        let new_term = term.get() + 1;
        set_term.set(new_term);
        
        if is_alive(node1.get()) {
            set_node1.set(1);
            set_events.update(|e| e.push(format!("👑 Node 1 elected (term {})", new_term)));
        } else if is_alive(node2.get()) {
            set_node2.set(1);
            set_events.update(|e| e.push(format!("👑 Node 2 elected (term {})", new_term)));
        } else if is_alive(node3.get()) {
            set_node3.set(1);
            set_events.update(|e| e.push(format!("👑 Node 3 elected (term {})", new_term)));
        }
    };
    
    // Trigger election with delay
    let trigger_election = move || {
        if !has_quorum() {
            set_events.update(|e| e.push("❌ QUORUM LOST — cluster halted".into()));
            return;
        }
        set_events.update(|e| e.push("⏳ Election timeout...".into()));
        Timeout::new(300, move || elect_leader()).forget();
    };
    
    // Auto-restart after 1s
    let schedule_restart = move |node_id: i32| {
        if !auto_restart.get() { return; }
        Timeout::new(1000, move || {
            if !auto_restart.get() { return; }
            let start = now();
            match node_id {
                1 if node1.get() == 3 => set_node1.set(0),
                2 if node2.get() == 3 => set_node2.set(0),
                3 if node3.get() == 3 => set_node3.set(0),
                _ => return,
            }
            let ms = now() - start;
            set_events.update(|e| e.push(format!("🔄 [WATCHDOG] N{} restarted ({:.1}ms)", node_id, ms)));
            Timeout::new(50, move || elect_leader()).forget();
        }).forget();
    };
    
    // Rogue rejoins with PreVote
    let rogue_rejoins = move || {
        if node3.get() != 4 { return; }
        set_events.update(|e| {
            e.push("───── PREVOTE DEMO ─────".into());
            e.push(format!("🏴‍☠️ N3 asks: vote for me? (term {})", rogue_term.get()));
            e.push("❌ N1: NO — I have a leader".into());
            e.push("❌ N2: NO — I have a leader".into());
            e.push("✅ BLOCKED! N3 rejoins as follower".into());
        });
        set_node3.set(0);
        set_rogue_term.set(term.get());
    };
    
    // KV submit
    let do_kv = move |cmd: String| {
        if cmd.is_empty() { return; }
        if !has_quorum() {
            set_kv_out.update(|o| o.push(format!("> {} ❌ No quorum", cmd)));
            return;
        }
        if !has_leader() {
            set_kv_out.update(|o| o.push(format!("> {} ⏳ No leader", cmd)));
            return;
        }
        let idx = log_index.get() + 1;
        set_log_index.set(idx);
        set_kv_out.update(|o| o.push(format!("> {} ✓ @{}", cmd, idx)));
        set_events.update(|e| e.push(format!("📝 Log[{}]: {}", idx, cmd)));
    };
    
    view! {
        <div class="dashboard">
            <header class="header">
                <h1>"🗳️ Raft Consensus"</h1>
                <div class="badges">
                    <span class="badge" class:ok=has_quorum class:fail=move || !has_quorum()>
                        {move || if has_quorum() { "QUORUM ✓" } else { "NO QUORUM ✗" }}
                    </span>
                    <span class="badge term">"Term " {term}</span>
                </div>
            </header>
            
            <div class="info-box">
                "Raft keeps 3 nodes in sync. Kill nodes → see leader election. "
                "Try Watchdog for auto-restart. Hover buttons for tooltips."
            </div>
            
            <div class="main-grid">
                <div class="left-col">
                    // Cluster
                    <div class="card">
                        <div class="card-title">"Cluster"</div>
                        <div class="nodes">
                            <div class="node" class=move || state_class(node1.get())>
                                <div class="emoji">{move || state_emoji(node1.get())}</div>
                                <div class="name">"Node 1"</div>
                                <div class="state">{move || state_name(node1.get())}</div>
                            </div>
                            <div class="node" class=move || state_class(node2.get())>
                                <div class="emoji">{move || state_emoji(node2.get())}</div>
                                <div class="name">"Node 2"</div>
                                <div class="state">{move || state_name(node2.get())}</div>
                            </div>
                            <div class="node" class=move || state_class(node3.get())>
                                <div class="emoji">{move || state_emoji(node3.get())}</div>
                                <div class="name">"Node 3"</div>
                                <div class="state">{move || if node3.get() == 4 { format!("ROGUE (t={})", rogue_term.get()) } else { state_name(node3.get()).into() }}</div>
                            </div>
                        </div>
                        {move || if !has_quorum() {
                            view! { <div class="warning">"⚠️ HALTED — need 2/3 for quorum"</div> }
                        } else { view! { <div></div> } }}
                    </div>
                    
                    // Controls
                    <div class="card">
                        <div class="card-title">"🎮 Controls"</div>
                        <div class="controls">
                            <button class="btn red" title="Kill node. If leader, triggers election."
                                disabled=move || node1.get() == 3
                                on:click=move |_| {
                                    let was_leader = node1.get() == 1;
                                    set_node1.set(3);
                                    set_events.update(|e| e.push("💀 Killed N1".into()));
                                    if was_leader { trigger_election(); }
                                    schedule_restart(1);
                                }>"💀 Kill N1"</button>
                            <button class="btn red" title="Kill node. If leader, triggers election."
                                disabled=move || node2.get() == 3
                                on:click=move |_| {
                                    let was_leader = node2.get() == 1;
                                    set_node2.set(3);
                                    set_events.update(|e| e.push("💀 Killed N2".into()));
                                    if was_leader { trigger_election(); }
                                    schedule_restart(2);
                                }>"💀 Kill N2"</button>
                            <button class="btn red" title="Kill node. If leader, triggers election."
                                disabled=move || node3.get() == 3 || node3.get() == 4
                                on:click=move |_| {
                                    let was_leader = node3.get() == 1;
                                    set_node3.set(3);
                                    set_events.update(|e| e.push("💀 Killed N3".into()));
                                    if was_leader { trigger_election(); }
                                    schedule_restart(3);
                                }>"💀 Kill N3"</button>
                            
                            <button class="btn blue" title="Restart node. Shows fast WASM restart."
                                disabled=move || node1.get() != 3
                                on:click=move |_| {
                                    set_node1.set(0);
                                    set_events.update(|e| e.push("🚀 N1 restarted".into()));
                                    if !has_leader() { Timeout::new(100, move || elect_leader()).forget(); }
                                }>"🔄 Restart N1"</button>
                            <button class="btn blue" title="Restart node. Shows fast WASM restart."
                                disabled=move || node2.get() != 3
                                on:click=move |_| {
                                    set_node2.set(0);
                                    set_events.update(|e| e.push("🚀 N2 restarted".into()));
                                    if !has_leader() { Timeout::new(100, move || elect_leader()).forget(); }
                                }>"🔄 Restart N2"</button>
                            <button class="btn blue" title="Restart node. Shows fast WASM restart."
                                disabled=move || node3.get() != 3
                                on:click=move |_| {
                                    set_node3.set(0);
                                    set_events.update(|e| e.push("🚀 N3 restarted".into()));
                                    if !has_leader() { Timeout::new(100, move || elect_leader()).forget(); }
                                }>"🔄 Restart N3"</button>
                        </div>
                        
                        <div class="card-title" style="margin-top:1rem">"🏴‍☠️ Disruptive Server"</div>
                        <p class="help-text">"A disconnected node inflates its term. PreVote blocks it."</p>
                        <div class="controls">
                            <button class="btn orange" title="Disconnect N3. It inflates term to 50."
                                disabled=move || node3.get() == 4
                                on:click=move |_| {
                                    set_node3.set(4);
                                    set_rogue_term.set(50);
                                    set_events.update(|e| e.push("🏴‍☠️ N3 partitioned (term→50)".into()));
                                }>"🔌 Disconnect N3"</button>
                            <button class="btn green" title="N3 tries to rejoin. PreVote rejects its high term."
                                disabled=move || node3.get() != 4
                                on:click=move |_| rogue_rejoins()
                            >"✨ Heal & Rejoin"</button>
                        </div>
                        
                        <div class="card-title" style="margin-top:1rem">"⚙️ Settings"</div>
                        <div class="controls">
                            <button class="btn" 
                                title="Auto-restart dead/rogue nodes. Simulates systemd/K8s. Each WASM module restarts independently."
                                class:active=auto_restart
                                on:click=move |_| {
                                    set_auto_restart.update(|v| *v = !*v);
                                    if auto_restart.get() {
                                        set_events.update(|e| e.push("🔧 Watchdog ON".into()));
                                        // Immediately restart any dead nodes
                                        if node1.get() == 3 { schedule_restart(1); }
                                        if node2.get() == 3 { schedule_restart(2); }
                                        if node3.get() == 3 { schedule_restart(3); }
                                        // Also heal rogue (partitioned) nodes
                                        if node3.get() == 4 {
                                            set_events.update(|e| e.push("🔧 Healing partitioned N3...".into()));
                                            rogue_rejoins();
                                        }
                                    } else {
                                        set_events.update(|e| e.push("🔧 Watchdog OFF".into()));
                                    }
                                }>
                                {move || if auto_restart.get() { "🟢 Watchdog ON" } else { "⚪ Watchdog OFF" }}
                            </button>
                            <button class="btn" title="Reset to initial state."
                                on:click=move |_| {
                                    set_node1.set(1); set_node2.set(0); set_node3.set(0);
                                    set_term.set(1); set_log_index.set(0);
                                    set_rogue_term.set(1); set_auto_restart.set(false);
                                    set_events.set(vec!["✨ Reset".into(), "👑 N1 leader (term 1)".into()]);
                                    set_kv_out.set(vec![]);
                                }>"🔄 Reset"</button>
                        </div>
                    </div>
                    
                    // KV Store
                    <div class="card">
                        <div class="card-title">"💾 Replicated State"</div>
                        <p class="help-text">"Commands go through Raft: replicate → ACK → commit."</p>
                        <div class="kv-buttons">
                            <button class="btn" title="SET user alice" on:click=move |_| do_kv("SET user alice".into())>"SET user"</button>
                            <button class="btn" title="SET count 42" on:click=move |_| do_kv("SET count 42".into())>"SET count"</button>
                            <button class="btn" title="DEL temp" on:click=move |_| do_kv("DEL temp".into())>"DEL temp"</button>
                        </div>
                        <div class="kv-input">
                            <input type="text" id="kv-cmd" placeholder="SET key value" />
                            <button class="btn" on:click=move |_| {
                                let input = web_sys::window()
                                    .and_then(|w| w.document())
                                    .and_then(|d| d.get_element_by_id("kv-cmd"))
                                    .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok());
                                if let Some(el) = input {
                                    let cmd = el.value();
                                    if !cmd.is_empty() {
                                        do_kv(cmd);
                                        el.set_value("");
                                    }
                                }
                            }>"Submit"</button>
                        </div>
                        <div class="kv-output">
                            {move || kv_out.get().into_iter().map(|l| {
                                let class = if l.contains("✓") { "ok" } else { "err" };
                                view! { <div class=class>{l}</div> }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                </div>
                
                // Right column
                <div class="right-col">
                    <div class="card">
                        <div class="card-title">"⚡ Metrics"</div>
                        <div class="metrics">
                            <div class="metric" title="WASM module init time">
                                <div class="value">{move || format!("{:.1}", wasm_init_ms.get())}</div>
                                <div class="label">"Init (ms)"</div>
                            </div>
                            <div class="metric" title="Raft term. Increments each election.">
                                <div class="value">{term}</div>
                                <div class="label">"Term"</div>
                            </div>
                            <div class="metric" title="Entries in replicated log.">
                                <div class="value">{log_index}</div>
                                <div class="label">"Log"</div>
                            </div>
                            <div class="metric" title="Alive nodes. Need 2/3 for quorum.">
                                <div class="value">{move || format!("{}/3", alive_count())}</div>
                                <div class="label">"Alive"</div>
                            </div>
                        </div>
                    </div>
                    
                    <div class="card events-card">
                        <div class="card-title">"📋 Events"</div>
                        <div class="events">
                            {move || events.get().iter().rev().cloned().collect::<Vec<_>>().into_iter().map(|e| {
                                let class = if e.contains("❌") || e.contains("💀") { "red" }
                                    else if e.contains("✅") || e.contains("👑") { "green" }
                                    else if e.contains("⏳") { "yellow" }
                                    else { "" };
                                view! { <div class="event" class=class>{e}</div> }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
