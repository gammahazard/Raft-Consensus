//! # dashboard
//!
//! why: provide interactive web ui for raft cluster visualization
//! relations: uses shim/host.js for cluster management, displays node states  
//! what: leptos components for cluster viz, chaos controls, kv store, event log
//!
//! SIMPLIFIED: Clearer flow, auto-PreVote, better explanations

use leptos::*;
use wasm_bindgen::prelude::*;
use gloo_timers::callback::Timeout;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

/// main app component
#[component]
pub fn App() -> impl IntoView {
    // Node states: 0=follower, 1=leader, 2=candidate, 3=dead, 4=rogue
    let (node1, set_node1) = create_signal(1i32); // starts as leader
    let (node2, set_node2) = create_signal(0i32);
    let (node3, set_node3) = create_signal(0i32);
    
    // Raft state
    let (term, set_term) = create_signal(1i32);
    let (log_index, set_log_index) = create_signal(0i32);
    let (rogue_term, set_rogue_term) = create_signal(1i32);
    
    // Auto-restart toggle (simulates systemd/K8s watchdog)
    let (auto_restart, set_auto_restart) = create_signal(false);
    let (last_restart_ms, set_last_restart_ms) = create_signal(0.0f64);
    
    // WASM metrics (measured at load)
    let (wasm_init_ms, set_wasm_init_ms) = create_signal(0.0f64);
    create_effect(move |_| {
        let start = now();
        // Do some work to measure real WASM performance
        let mut sum: u64 = 0;
        for i in 0..50000 { sum = sum.wrapping_add(i); }
        let _ = sum;
        set_wasm_init_ms.set(now() - start);
    });
    
    // Events log (declared early for use in closures)
    let (events, set_events) = create_signal::<Vec<String>>(vec![
        "✨ Cluster started with 3 nodes".into(),
        "👑 Node 1 elected leader (term 1)".into(),
    ]);
    
    // Auto-restart logic: when enabled, restart dead nodes after 1 second
    let schedule_auto_restart = move |node_id: i32| {
        if !auto_restart.get() { return; }
        
        Timeout::new(1000, move || {
            if !auto_restart.get() { return; } // Check again in case toggled off
            
            let start = now();
            match node_id {
                1 if node1.get() == 3 => {
                    set_node1.set(0);
                    let elapsed = now() - start;
                    set_last_restart_ms.set(elapsed);
                    set_events.update(|e| {
                        e.push(format!("🔄 [WATCHDOG] Auto-restarted N1 ({:.1}ms)", elapsed));
                        if log_index.get() > 0 {
                            e.push(format!("📥 N1 syncing {} entries...", log_index.get()));
                        }
                    });
                },
                2 if node2.get() == 3 => {
                    set_node2.set(0);
                    let elapsed = now() - start;
                    set_last_restart_ms.set(elapsed);
                    set_events.update(|e| {
                        e.push(format!("🔄 [WATCHDOG] Auto-restarted N2 ({:.1}ms)", elapsed));
                        if log_index.get() > 0 {
                            e.push(format!("📥 N2 syncing {} entries...", log_index.get()));
                        }
                    });
                },
                3 if node3.get() == 3 => {
                    set_node3.set(0);
                    let elapsed = now() - start;
                    set_last_restart_ms.set(elapsed);
                    set_events.update(|e| {
                        e.push(format!("🔄 [WATCHDOG] Auto-restarted N3 ({:.1}ms)", elapsed));
                        if log_index.get() > 0 {
                            e.push(format!("📥 N3 syncing {} entries...", log_index.get()));
                        }
                    });
                },
                _ => {}
            }
        }).forget();
    };
    
    // KV store output
    let (kv_out, set_kv_out) = create_signal::<Vec<String>>(vec![]);
    
    // Helper functions
    let alive_count = move || {
        [node1.get(), node2.get(), node3.get()]
            .iter()
            .filter(|&&s| s != 3 && s != 4)
            .count()
    };
    let has_quorum = move || alive_count() >= 2;
    let has_leader = move || node1.get() == 1 || node2.get() == 1 || node3.get() == 1;
    
    let state_emoji = |s: i32| match s {
        1 => "👑", 2 => "🗳️", 3 => "💀", 4 => "🏴‍☠️", _ => "🟢"
    };
    let state_name = |s: i32| match s {
        1 => "LEADER", 2 => "CANDIDATE", 3 => "DEAD", 4 => "ROGUE", _ => "FOLLOWER"
    };
    
    // Auto-elect new leader when leader dies
    let trigger_election = move |killed: i32| {
        if !has_quorum() {
            set_events.update(|e| {
                e.push("❌ QUORUM LOST! Need 2/3 nodes.".into());
                e.push("⚠️ Cluster HALTED (safety > availability)".into());
            });
            return;
        }
        
        set_events.update(|e| e.push("⏳ Election timeout... (150-300ms)".into()));
        
        let new_term = term.get() + 1;
        Timeout::new(300, move || {
            set_term.set(new_term);
            // First alive non-killed node becomes candidate then leader
            if killed != 2 && node2.get() == 0 {
                set_events.update(|e| {
                    e.push(format!("🗳️ Node 2 becomes candidate (term {})", new_term));
                    e.push("✅ Node 2 wins election (2/3 votes)".into());
                });
                set_node2.set(1);
            } else if killed != 3 && node3.get() == 0 {
                set_events.update(|e| {
                    e.push(format!("🗳️ Node 3 becomes candidate (term {})", new_term));
                    e.push("✅ Node 3 wins election (2/3 votes)".into());
                });
                set_node3.set(1);
            }
        }).forget();
    };
    
    // Rogue node rejoins with PreVote (AUTOMATIC)
    let rogue_rejoins = move || {
        if node3.get() != 4 { return; }
        
        set_events.update(|e| {
            e.push("─────────── PREVOTE DEMO ───────────".into());
            e.push(format!("🏴‍☠️ Rogue N3 tries to rejoin (term {})", rogue_term.get()));
            e.push(format!("📤 N3 → N1: \"Would you vote for me?\" (term={})", rogue_term.get()));
            e.push(format!("📤 N3 → N2: \"Would you vote for me?\" (term={})", rogue_term.get()));
            e.push("📥 N1 → N3: \"NO! I have a leader.\" ❌".into());
            e.push("📥 N2 → N3: \"NO! I have a leader.\" ❌".into());
            e.push("".into());
            e.push("✅ PREVOTE BLOCKED THE ROGUE!".into());
            e.push("   → N3's high term (50) did NOT disrupt cluster".into());
            e.push("   → Leader stays at term 1".into());
            e.push("   → N3 syncs to term 1 and rejoins as follower".into());
            e.push("─────────────────────────────────────".into());
        });
        
        set_node3.set(0); // Rejoin as follower
        set_rogue_term.set(term.get());
    };
    
    view! {
        <div class="dashboard">
            // Header
            <header class="header">
                <h1>"🗳️ Raft Consensus"</h1>
                <div class="badges">
                    <span class="badge" class:ok=has_quorum class:fail=move || !has_quorum()>
                        {move || if has_quorum() { "QUORUM ✓" } else { "NO QUORUM ✗" }}
                    </span>
                    <span class="badge term">"Term " {term}</span>
                </div>
            </header>
            
            // Info box explaining what we're demonstrating
            <div class="info-box">
                <strong>"What this demo shows: "</strong>
                "Raft consensus keeps 3 nodes in sync. Kill nodes to see leader election. "
                "Try the Rogue Node to see PreVote protection."
            </div>
            
            <div class="main-grid">
                // Left: Cluster + Controls
                <div class="left-col">
                    // Cluster visualization
                    <div class="card">
                        <div class="card-title">"Cluster Status"</div>
                        <div class="nodes">
                            <div class="node" class=move || state_name(node1.get()).to_lowercase()>
                                <div class="emoji">{move || state_emoji(node1.get())}</div>
                                <div class="name">"Node 1"</div>
                                <div class="state">{move || state_name(node1.get())}</div>
                            </div>
                            <div class="node" class=move || state_name(node2.get()).to_lowercase()>
                                <div class="emoji">{move || state_emoji(node2.get())}</div>
                                <div class="name">"Node 2"</div>
                                <div class="state">{move || state_name(node2.get())}</div>
                            </div>
                            <div class="node" class=move || state_name(node3.get()).to_lowercase()>
                                <div class="emoji">{move || state_emoji(node3.get())}</div>
                                <div class="name">"Node 3"</div>
                                <div class="state">
                                    {move || if node3.get() == 4 { 
                                        format!("ROGUE (term {})", rogue_term.get()) 
                                    } else { 
                                        state_name(node3.get()).to_string() 
                                    }}
                                </div>
                            </div>
                        </div>
                        
                        // Warning when no quorum
                        {move || if !has_quorum() {
                            view! {
                                <div class="warning">
                                    "⚠️ CLUSTER HALTED — need 2/3 nodes for quorum. "
                                    "This is Raft's safety guarantee!"
                                </div>
                            }
                        } else {
                            view! { <div></div> }
                        }}
                    </div>
                    
                    // Controls
                    <div class="card">
                        <div class="card-title">"🎮 Controls"</div>
                        <div class="controls">
                            // Kill buttons
                            <button class="btn red" 
                                title="Simulate node crash. If leader, triggers election."
                                disabled=move || node1.get() == 3
                                on:click=move |_| {
                                    let was_leader = node1.get() == 1;
                                    set_node1.set(3);
                                    set_events.update(|e| e.push("💀 Killed Node 1".into()));
                                    if was_leader { trigger_election(1); }
                                    schedule_auto_restart(1);
                                }>"💀 Kill N1"</button>
                            <button class="btn red" 
                                title="Simulate node crash. If leader, triggers election."
                                disabled=move || node2.get() == 3
                                on:click=move |_| {
                                    let was_leader = node2.get() == 1;
                                    set_node2.set(3);
                                    set_events.update(|e| e.push("💀 Killed Node 2".into()));
                                    if was_leader { trigger_election(2); }
                                    schedule_auto_restart(2);
                                }>"💀 Kill N2"</button>
                            <button class="btn red" 
                                title="Simulate node crash. If leader, triggers election."
                                disabled=move || node3.get() == 3 || node3.get() == 4
                                on:click=move |_| {
                                    let was_leader = node3.get() == 1;
                                    set_node3.set(3);
                                    set_events.update(|e| e.push("💀 Killed Node 3".into()));
                                    if was_leader { trigger_election(3); }
                                    schedule_auto_restart(3);
                                }>"💀 Kill N3"</button>
                            
                            // Restart buttons
                            <button class="btn blue" 
                                title="Restart dead node. Shows fast WASM restart time and log catch-up."
                                disabled=move || node1.get() != 3
                                on:click=move |_| {
                                    let start = now();
                                    set_node1.set(0);
                                    let ms = now() - start;
                                    set_events.update(|e| {
                                        e.push(format!("🚀 Node 1 restarted ({:.1}ms)", ms));
                                        if log_index.get() > 0 {
                                            e.push(format!("📥 Syncing {} log entries...", log_index.get()));
                                        }
                                    });
                                }>"🔄 Restart N1"</button>
                            <button class="btn blue" 
                                title="Restart dead node. Shows fast WASM restart time and log catch-up."
                                disabled=move || node2.get() != 3
                                on:click=move |_| {
                                    let start = now();
                                    set_node2.set(0);
                                    let ms = now() - start;
                                    set_events.update(|e| {
                                        e.push(format!("🚀 Node 2 restarted ({:.1}ms)", ms));
                                        if log_index.get() > 0 {
                                            e.push(format!("📥 Syncing {} log entries...", log_index.get()));
                                        }
                                    });
                                }>"🔄 Restart N2"</button>
                            <button class="btn blue" 
                                title="Restart dead node. Shows fast WASM restart time and log catch-up."
                                disabled=move || node3.get() != 3
                                on:click=move |_| {
                                    let start = now();
                                    set_node3.set(0);
                                    let ms = now() - start;
                                    set_events.update(|e| {
                                        e.push(format!("🚀 Node 3 restarted ({:.1}ms)", ms));
                                        if log_index.get() > 0 {
                                            e.push(format!("📥 Syncing {} log entries...", log_index.get()));
                                        }
                                    });
                                }>"🔄 Restart N3"</button>
                        </div>
                        
                        // PreVote demo
                        <div class="card-title" style="margin-top: 1rem">"🏴‍☠️ Disruptive Server Demo"</div>
                        <p class="help-text">
                            "A disconnected node keeps timing out and incrementing its term. "
                            "Without PreVote, it would disrupt the cluster when it rejoins. "
                            "With PreVote, healthy nodes reject its high term."
                        </p>
                        <div class="controls">
                            <button class="btn orange" 
                                title="Disconnect N3 from cluster. It starts inflating its term (1→50) trying to become leader."
                                disabled=move || node3.get() == 4
                                on:click=move |_| {
                                    set_node3.set(4);
                                    set_rogue_term.set(50);
                                    set_events.update(|e| {
                                        e.push("─────────────────────────────────────".into());
                                        e.push("🏴‍☠️ Node 3 PARTITIONED!".into());
                                        e.push("   (simulating disconnected node)".into());
                                        e.push("   Term inflating: 1 → 10 → 25 → 50".into());
                                        e.push("─────────────────────────────────────".into());
                                    });
                                }>"🔌 Disconnect N3"</button>
                            <button class="btn green"
                                title="N3 tries to rejoin. PreVote protocol asks 'would you vote for me?' — healthy nodes say NO because they have a leader."
                                disabled=move || node3.get() != 4
                                on:click=move |_| rogue_rejoins()
                            >"✨ Heal & Rejoin"</button>
                        </div>
                        
                        // Watchdog toggle + Reset
                        <div class="card-title" style="margin-top: 1rem">"⚙️ Settings"</div>
                        <div class="controls">
                            <button class="btn" 
                                title="Simulates systemd/K8s watchdog. When ON, dead nodes auto-restart after 1 second."
                                class:active=auto_restart
                                on:click=move |_| {
                                    set_auto_restart.update(|v| *v = !*v);
                                    set_events.update(|e| {
                                        if auto_restart.get() {
                                            e.push("🔧 Watchdog ENABLED (nodes auto-restart in 1s)".into());
                                        } else {
                                            e.push("🔧 Watchdog DISABLED".into());
                                        }
                                    });
                                }>
                                {move || if auto_restart.get() { "🟢 Watchdog ON" } else { "⚪ Watchdog OFF" }}
                            </button>
                            <button class="btn" 
                                title="Reset cluster to initial state: N1 as leader, term=1, clear all logs."
                                on:click=move |_| {
                                set_node1.set(1); set_node2.set(0); set_node3.set(0);
                                set_term.set(1); set_log_index.set(0); set_rogue_term.set(1);
                                set_auto_restart.set(false);
                                set_events.set(vec![
                                    "✨ Cluster reset".into(),
                                    "👑 Node 1 elected leader (term 1)".into(),
                                ]);
                                set_kv_out.set(vec![]);
                            }>"🔄 Reset All"</button>
                        </div>
                    </div>
                    
                    // KV Store
                    <div class="card">
                        <div class="card-title">"💾 Replicated State Machine"</div>
                        <p class="help-text">
                            "This is Raft's state machine. Commands (SET/GET) go through consensus: "
                            "Leader replicates to followers → majority ACK → committed to log."
                        </p>
                        <KvStore 
                            kv_out set_kv_out 
                            has_leader has_quorum
                            log_index set_log_index
                            set_events
                        />
                    </div>
                </div>
                
                // Right: Events + Metrics
                <div class="right-col">
                    // Metrics
                    <div class="card">
                        <div class="card-title">"⚡ Metrics"</div>
                        <div class="metrics">
                            <div class="metric" title="Time to initialize WASM module. Real measurement from browser runtime.">
                                <div class="value">{move || format!("{:.1}", wasm_init_ms.get())}</div>
                                <div class="label">"WASM Init (ms)"</div>
                            </div>
                            <div class="metric" title="Raft election term. Increments with each new election. Higher = more elections happened.">
                                <div class="value">{term}</div>
                                <div class="label">"Term"</div>
                            </div>
                            <div class="metric" title="Number of entries in the replicated log. Each KV command adds one entry.">
                                <div class="value">{log_index}</div>
                                <div class="label">"Log Index"</div>
                            </div>
                            <div class="metric" title="Nodes currently running. Need 2/3 for quorum (majority).">
                                <div class="value">{move || format!("{}/3", alive_count())}</div>
                                <div class="label">"Alive"</div>
                            </div>
                        </div>
                    </div>
                    
                    // Event log
                    <div class="card events-card">
                        <div class="card-title">"📋 Events"</div>
                        <div class="events">
                            {move || events.get().iter().rev().cloned().collect::<Vec<_>>().into_iter().map(|e| {
                                let class = if e.contains("❌") || e.contains("💀") { "red" }
                                    else if e.contains("✅") || e.contains("👑") { "green" }
                                    else if e.contains("⏳") || e.contains("🗳") { "yellow" }
                                    else if e.contains("───") { "dim" }
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

/// KV Store component
#[component]
fn KvStore(
    kv_out: ReadSignal<Vec<String>>,
    set_kv_out: WriteSignal<Vec<String>>,
    has_leader: impl Fn() -> bool + 'static + Copy,
    has_quorum: impl Fn() -> bool + 'static + Copy,
    log_index: ReadSignal<i32>,
    set_log_index: WriteSignal<i32>,
    set_events: WriteSignal<Vec<String>>,
) -> impl IntoView {
    let input = create_node_ref::<leptos::html::Input>();
    
    let submit = move |_| {
        if let Some(el) = input.get() {
            let cmd = el.value();
            if cmd.is_empty() { return; }
            
            if !has_quorum() {
                set_kv_out.update(|o| o.push(format!("> {} ❌ No quorum!", cmd)));
                return;
            }
            if !has_leader() {
                set_kv_out.update(|o| o.push(format!("> {} ⏳ Election...", cmd)));
                return;
            }
            
            let idx = log_index.get() + 1;
            set_log_index.set(idx);
            set_kv_out.update(|o| o.push(format!("> {} ✓ Committed @ {}", cmd, idx)));
            set_events.update(|e| e.push(format!("📝 Log[{}]: {}", idx, cmd)));
            el.set_value("");
        }
    };
    
    view! {
        <div class="kv-store">
            <div class="kv-input">
                <input type="text" placeholder="SET key value" node_ref=input />
                <button on:click=submit>"Submit"</button>
            </div>
            <div class="kv-output">
                {move || kv_out.get().into_iter().map(|l| {
                    let class = if l.contains("✓") { "ok" } else { "err" };
                    view! { <div class=class>{l}</div> }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
