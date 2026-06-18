---
title: "Avalanche Design Proposal"
date: 2026-05-20
author: "Lincoln Quirk"
description: "Why we're building a new organizing platform"
---

# **Proposal**

We'll start with a secure messaging platform, standing on the shoulders of Signal. On top of that, we'll build a powerful organizer toolkit, enabling organizers to build whatever they need to activate and nurture their own communities:

* People will install the app because a specific action they're participating in (an action, a conference, etc) requires it.  
* But they'll stick around because the network captures and represents the real social connections they formed.

Lincoln has started working on it here: [https://github.com/lincolnq/avalanche](https://github.com/lincolnq/avalanche)  

## **Why this might be worth it**

There are a lot of drawbacks to making people install a new app, but there are also huge potential upsides. We intend to overcome the drawbacks while providing access to the upsides.

Let's take the example of a conference messaging tool. We want to enable anyone at a conference to find each other's contact info by default. Conferences are high-investment events (people are committing a full day minimum or more) to the conference, and most "conference apps" (Swapcard, Whova, etc) kinda suck. One of the main ways they suck is that they're not messaging first—messaging is an afterthought—and as a result it's quite unreliable to actually get in touch with people on the platform.

If conferences were organized on Avalanche instead, they would:

* Be able to sign up afresh for an identity on Avalanche, or reuse their identity from a previous conference  
* Have Signal-quality messaging (including encrypted comms) for DMs and small groups  
* Automatically be invited to announcements channels as well as listing public chatter channels to join  
* Within the same app, have conference schedule tools with features customizable on a per-conference basis

The design centers on self-hosted Signal-quality encrypted messaging — a unified inbox of all your conversations across *all* your activism and associated socials — with a project platform to enable anyone to rapidly build organizing tools that are specifically directly integrated into the messaging.

The project platform will enable projects like:

* Any server specific onboarding things:  
  * onboarding questions  
  * vetting flow  
  * Default joined channels \+ channel lists  
  * Allowing people to join and leave large channels on their own, or self sort into smaller channels  
* For action groups: Automated team assignment directly into secure group chats; app with maps, comms and location tracker \- but it's not its own app, it's just a Project  
* For conferences: Conference attendee list; schedule; channel browser  
* Serverwide profile tools — some groups may want to implement some form of social currency/karma, or public thanks (like wikipedia's barnstars), or just different profile fields  
* Bots that answer questions, that are well integrated into the platform and supported  
* Moderation bots  
* Bots that can monitor activity levels and show which discussions need more energy/help injected  
* Forums (e.g., persistent, threaded, rather than ephemeral discussions) attached to your activist identity  
* Secure shared editable docs attached to your activist identity

The Avalanche platform is self-hosted and open source. Specifically, any group who wants to set up the platform controls their own "server": think a Slack server or a Discord server, but even more under the group's control because the platform is open source and (optionally) self hosted. Groups can set standards for who is invited to the server and what kinds of automatic access they have.

Similar to Slack or Discord, there will only be one shared Avalanche app to download, because getting people to download a new messaging app is quite difficult & costly — so trying to make the app as general-purpose as possible, in order to not need to solve this problem again in the future\!

## **Project Plan**

We can build this relatively quickly because Signal has open-sourced most if not all of their platform, and Claude can pick up from where Signal left off. We can use libsignal as the basis of our secure communication system, rebuild all messaging UX on top of it, and then add on the Project platform.

I (Lincoln) have been soloing this for the last several weeks using Claude, but I've only had time to work on it a few hours per week. I have gotten fairly far, but there's still a lot to do.