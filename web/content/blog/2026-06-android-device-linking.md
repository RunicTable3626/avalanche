---
title: "Android App & Device Linking"
date: 2026-06-27
author: "Lincoln Quirk"
description: "Release 0.3.0"
---

Two big things landed this week: Avalanche now runs on Android, and you can now link a device to your existing Avalanche accounts, like Signal Desktop.

**Calling for testers**---The process for being a tester is simplified quite a bit now (although still not as easy as installing from an app store) so if you're excited about Avalanche, now's a great time to try it and give some feedback!

<p>
  <a class="btn btn-primary" href="/getting-started/sideload-android/" rel="noopener">Try Android</a>
  <a class="btn btn-primary" href="/getting-started/testflight-ios/" rel="noopener">Try iOS</a>
</p>

## Android is here!

Building the Android app was my top priority last week, and it's now pretty much caught up with the iPhone app.

Avalanche isn't yet on the Google Play Store, so on Android you'll have to install it directly from our website. This is called "side-loading" and I wrote up a [quick explainer here](/getting-started/sideload-android/).

## Device linking

One of Signal's best features is the ability to view your message inbox natively on desktop and have it sync with your mobile app. Avalanche now has the same capabilities; the desktop app isn't yet built (more on that below) but you can link any number of devices to one account. Each of those devices will have the same privileges and see the same messages as the original device, just like Signal.

Signal's model constrains you to having the mobile app as your main device and then desktop follows it. With Avalanche there is no such constraint---you can link devices in any order.

As always, all of this stays end-to-end encrypted. Linking a device does expose you from a security perspective a little bit, since anyone who manages to get into one of your devices can read your messages, but for many users it's a worthwhile trade-off.

## iOS TestFlight open alpha

Up till now, testing the iOS app has needed you to be manually approved by Lincoln. This is no longer true! [Anyone can sign up to be a tester now via TestFlight.](/getting-started/testflight-ios/) We are currently limited to 50 testers, but if we run through that then I'll find another solution!

Because of this and the Android sideloading capability, almost any device should be able to try Avalanche now. Let Lincoln know if you have any issues getting started.

## Desktop progress

Device linking is important groundwork for the desktop app that's coming. We've made a lot of progress on it, but it's not ready for release quite yet. 

On the technical side, we're using a tool called [Tauri](https://tauri.app/) for building our desktop app, which should be faster and less memory-intensive than Electron (what Signal and most desktop apps are built with).

Stay tuned :)

## Other improvements

* Push notifications work better. 
* The app should more consistently reconnect properly when opening after a long time in the background.
* When the app is relaunched it will drop you directly into the chats view, rather than showing the splash screen for a second
* "Unknown" names should be much less prevalent -- please report any names that stay "Unknown" for more than a second or two!
* You can delete your account!
