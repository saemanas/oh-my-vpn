---
topic: "Oh My VPN -- Hetzner-based on-demand VPN automation"
status: complete
idea_count: 50
cluster_count: 7
date: 2026-03-03
---

# Brainstorming: Oh My VPN

## 1. Session Summary

| Field | Value |
| --- | --- |
| Topic | On-demand VPN server automation using cloud providers (Hetzner, AWS, GCP) with a macOS menu bar app |
| Total Ideas | 50 |
| Clusters | 7 (all starred) |
| Techniques Used | Free Association, Role Storming, Assumption Mapping, Pain Chain, Jobs to Be Done, How Might We |

## 2. Research Findings

Research conducted before ideation to establish technical feasibility.

### A. Self-hosted VPN Software

| Software | License | Notes |
| --- | --- | --- |
| **WireGuard** | GPLv2 | Fastest, simplest, Linux kernel built-in. **Selected.** |
| OpenVPN | GPLv2 | Older, broader compatibility but slower |
| Algo VPN | AGPLv3 | Easy setup by Trail of Bits |
| Firezone | Apache 2.0 | WireGuard-based with web UI |

### B. Cloud Provider Automation

#### a. Hetzner

- Hourly billing: CAX11 (ARM) = EUR 0.0053/hr (~$0.0056/hr)
- CLI: `hcloud server create` / `hcloud server delete`
- cloud-init support: auto-install WireGuard on boot
- Pre-built WireGuard image: `--image wireguard`
- Regions: Germany (Falkenstein, Nuremberg), Finland (Helsinki), US (Ashburn, Hillsboro), Singapore

#### b. AWS

- Hourly billing: t4g.nano (ARM, 2vCPU/0.5GB) = $0.0042/hr
- CLI: `aws ec2 run-instances` / `aws ec2 terminate-instances`
- User data (cloud-init) support: auto-install WireGuard on boot
- No pre-built WireGuard image (use Ubuntu + user data script)
- Regions: 30+ regions worldwide (us-east-1, eu-west-1, ap-northeast-1, etc.)
- Free Tier: t2.micro 750hrs/month for 12 months

#### c. GCP

- Hourly billing: e2-micro (shared 2vCPU/1GB) = $0.0084/hr
- CLI: `gcloud compute instances create` / `gcloud compute instances delete`
- Startup script support: auto-install WireGuard on boot
- No pre-built WireGuard image (use Ubuntu + startup script)
- Regions: 42 regions worldwide (us-central1, europe-west1, asia-northeast1, etc.)
- Free Tier: e2-micro 1 instance always free (us-west1, us-central1, us-east1 only)

### C. Cost Comparison

| Scenario | Hetzner (CAX11) | AWS (t4g.nano) | GCP (e2-micro) | VPN Subscription |
| --- | --- | --- | --- | --- |
| Hourly rate | $0.0056/hr | $0.0042/hr | $0.0084/hr | -- |
| 1hr/day x 30 days | **$0.17/month** | **$0.13/month** | **$0.25/month** | **$5--12/month** |
| 4hr/day x 30 days | **$0.67/month** | **$0.50/month** | **$1.01/month** | **$5--12/month** |
| Regions | 6 | 30+ | 42 | varies |

Savings: 80--98% compared to subscription VPN across all providers.
AWS is cheapest per hour, GCP has most regions, Hetzner has simplest setup (pre-built WireGuard image).

### D. Existing Similar Projects

| Project | Approach |
| --- | --- |
| [vpn-on-demand](https://github.com/aa8855/vpn-on-demand) | Terraform + Ansible + GitHub Actions |
| [sarg.org.ru blog](http://sarg.org.ru/blog/hetzner-vpn/) | Pure shell + cloud-init |
| [ServerlessVPN](https://serverlessvpn.com/) | $49 paid, Docker-based, multi-cloud |
| Hetzner WireGuard App | `--image wireguard` with web UI |

## 3. Starred Clusters

All 7 clusters starred.

### ★ A. Core Infrastructure

| # | Idea |
| --- | --- |
| 1 | Open-source project |
| 2 | Use WireGuard as VPN engine |
| 3 | Hetzner hcloud-based VPS auto-provisioning |
| 4 | Multi-cloud support (Hetzner, GCP, AWS) |
| 5 | Per-provider account linking / login flow |
| 6 | API key/token direct registration |
| 19 | Call hcloud CLI internally from the app |

### ★ B. macOS App & Tech Stack

| # | Idea |
| --- | --- |
| 13 | macOS-only menu bar app (main interface) |
| 18 | Tauri-based (TS frontend + Rust backend) |
| 46 | Install via `brew install` |
| 47 | macOS dark/light mode auto-adaptation |
| 45 | Update settings -- auto / notify-only / manual |

### ★ C. Connection & Server Management

| # | Idea |
| --- | --- |
| 14 | Region select -> one-click server creation/connection from menu bar |
| 17 | Disconnect & Destroy one-click |
| 31 | Auto-cleanup on server creation failure + retry |
| 32 | Auto-reconnect on VPN disconnection |
| 49 | Simple mode (auto defaults) vs advanced mode (custom instance/OS/firewall) |

### ★ D. Security & Network

| # | Idea |
| --- | --- |
| 33 | Sensitive data (API keys, SSH keys) -- Tauri plugin for OS-level encrypted storage |
| 34 | General settings -- config file storage |
| 38 | Kill Switch -- block internet when VPN disconnects |
| 39 | Split Tunneling -- per-app/domain VPN routing |

### ★ E. Multi-device & Mobile

| # | Idea |
| --- | --- |
| 10 | QR code generation for iPhone/mobile connection |
| 11 | Mac auto-applies WireGuard config |
| 12 | Use official WireGuard client apps (no custom client) |
| 15 | QR code popup from menu bar -> iPhone pairing |
| 35 | Multi-device support -- multiple devices on 1 server (WireGuard peers) |
| 36 | Connected device list in menu bar |
| 37 | Per-device QR code generation |

### ★ F. Cost & Auto-shutdown

| # | Idea |
| --- | --- |
| 16 | Current session status display (connection, IP, cost) |
| 25 | Auto-shutdown timer |
| 26 | Idle detection -- alert when no traffic |
| 28 | Real-time session cost display |
| 29 | Auto VPN shutdown on Mac lock/sleep |
| 30 | Daily/monthly cost cap -- auto-shutdown on exceed |
| 43 | Session history -- date, region, duration, cost |
| 44 | Monthly cost report (per provider, per region) |

### ★ G. UX & Onboarding

| # | Idea |
| --- | --- |
| 7 | Geo-restricted content bypass (Netflix, YouTube, etc.) |
| 8 | Country-restricted site access |
| 22 | Provider sign-up page direct links (in-app -> browser) |
| 23 | API key issuance guide -- official docs links |
| 24 | First-run onboarding flow |
| 27 | Menu bar icon clearly shows VPN status |
| 40 | Global keyboard shortcut -- VPN connect/disconnect toggle |
| 41 | macOS native notifications -- "Server ready", "Connected", "Disconnected" |
| 42 | Notifications in English only |
| 48 | Debug log viewer (in-app) |
| 50 | First-run onboarding -- provider select -> key input -> first connection guide |

## 4. All Ideas (Raw)

1. Open-source project
2. Use WireGuard as VPN engine
3. Hetzner hcloud-based VPS auto-provisioning
4. Multi-cloud support (Hetzner, GCP, AWS)
5. Per-provider account linking / login flow
6. API key/token direct registration
7. Geo-restricted content bypass
8. Country-restricted site access
9. CLI-based interface
10. QR code generation for iPhone/mobile connection
11. Mac auto-applies WireGuard config
12. Use official WireGuard client apps
13. macOS-only menu bar app (main interface)
14. Region select -> one-click server creation/connection from menu bar
15. QR code popup from menu bar -> iPhone pairing
16. Current session status display (connection, IP, cost)
17. Disconnect & Destroy one-click
18. Tauri-based macOS menu bar app (TS frontend + Rust backend)
19. Call hcloud CLI internally from the app
20. Cloud account sign-up guide for new users (in-app or link)
21. Per-provider step-by-step walkthrough from sign-up to API key
22. Provider sign-up page direct links (in-app -> browser)
23. API key issuance guide -- official docs links
24. First-run onboarding flow: provider select -> account check -> sign-up or key input
25. Auto-shutdown timer (e.g., auto disconnect & destroy after 1 hour)
26. Idle detection -- alert when no VPN traffic
27. Menu bar icon color/animation for clear VPN status
28. Real-time session cost display (EUR 0.02 running...)
29. Auto VPN shutdown on Mac lock/sleep option
30. Daily/monthly cost cap -- auto-shutdown on exceed
31. Auto-cleanup on server creation failure + retry
32. Auto-reconnect on VPN disconnection
33. Sensitive data (API keys, SSH keys) -- Tauri plugin for OS-level encrypted storage
34. General settings (preferred region, timer, etc.) -- config file storage
35. Multi-device support -- multiple devices on 1 server via WireGuard peers
36. Connected device list visible in menu bar
37. Per-device QR code generation
38. Kill Switch -- block internet when VPN disconnects (prevent IP leak)
39. Split Tunneling -- per-app/domain VPN routing selection
40. Global keyboard shortcut -- VPN connect/disconnect toggle
41. macOS native notifications -- "Server ready", "Connected", "Disconnected"
42. Notifications in English only
43. Session history -- date, region, duration, cost logging
44. Monthly cost report (per provider, per region)
45. Update settings -- auto update / notify only / manual check
46. Install via `brew install`
47. macOS dark/light mode auto-adaptation
48. Debug log viewer (in-app)
49. Simple mode (auto defaults) vs advanced mode (custom instance/OS/firewall)
50. First-run onboarding -- provider select -> key input -> first connection guide
