---
title: "Getting started as an organizer"
---
Welcome to Avalanche! This is the organizer page: Here we'll lay out why bother switching to Avalanche, and the steps to get your own Avalanche server set up and onboard your team.

## What Avalanche is

Avalanche is designed to be the most _practical_ organizing platform out there. Fundamentally, the vast majority of your team (volunteers or community members) will use Avalanche just like they use Signal or WhatsApp: a single inbox where messages arrive and they can respond. Simplicity for the end user is what makes Avalanche practical; but as an organizer you also have requirements about onboarding, formations, communications channels and so on; and that's why simply Signal or Whatsapp on its own isn't sufficient.

Throughout the rest of this document I'm going to lay out bits of complexity we introduce atop out-of-the-box messaging platforms, like Signal or WhatsApp. Each bit of complexity is justified by why it helps you as an organizer over the long term:

### Avalanche vs Signal

Your users don't register with phone numbers; they register using an invite token (link or QR code) and passkey. It's really simple and works well -- you'll see it in action the first time you use the product! The invite token scheme allows you to control who signs-up, and the passkey means that there's a way for users to securely recover their account even if they lose their device.

When a user joins, they can be automatically added to whatever groups/channels you want. This is configurable based on who invited them, what roles you assign the user, or anything else.

Avalanche allows for powerful automation, primarily in the form of _bots_ as first-class citizens of your server. Bots can do anything a human can do: monitor and invite people to groups, answer questions via DM, vet and invite participants, and so on. You can write bot code yourself, or ask a friend or AI to help. 

With Avalanche, you'll be able to build and customize a wide variety of software tools and integrations, connected to your users' Avalanche identity. Through the "project framework", the platform will support powerful organizing tools like:

* sign-up forms and vetting flows
* checklists and training tools
* a database of all your users and oversight of their progress along their checklists
* checkable schedules or other up-to-the-minute personalized information that come directly from the database
* connecting bots to the database (e.g. letting users send photos of their recent action or protest and having that auto-categorized and placed into the db)
* dashboards to measure goals across the organization

These tools are mostly for you and your team to build and customize (rather than prebuilt for you), because the workflows substantially differ on a per-organizational basis. We can give advice for figuring out how to design and implement your organization's workflow within Avalanche, especially with help from AI coding tools.

## Self Hosting and Security

We need to explain a few technical aspects of Avalanche that impact your team and some decisions you may have to make.

Avalanche is a private communications app. One of the main goals of Avalanche security is _robustness against server seizure_: the scenario where an adversary is showing up at your server's datacenter and physically hacking in, dumping the data, swapping out the code and so on. If that happens (if Avalanche has done its job), your users will still be protected: their private communications still won't be leaked and the adversary won't even be able to tell who is talking to whom.

To achieve this end, Avalanche is _open-source_, _self-hosted_ and _encrypted_: 

**Open-source** means it's possible to do your own technical audit of all the code if you wish and fix bugs. This mostly just enables you to believe us when we say that we've implemented everything in a secure way, and has few drawbacks; the main drawback is just that it is harder for a central Avalanche organization to make money; but we are currently not trying or planning on making money.[^avlicense]

[^avlicense]: Avalanche is licensed under the Affero GPL, just like Signal's source code.

**Self-hosted** means that you are in control of your own deployment and can deploy your own version if necessary. Self-hosted rather than relying on a centralized cloud service also limits the blast radius of any potential takedowns and ensures a strong trust chain between your users and their data. We'll flag that self-hosting in particular has a substantial complexity cost and that lands on _you_, the organizer, who must provide the infrastructure and technical expertise to deploy and secure your Avalanche server. We think this trade-off is worth it long-term _to you_ for the control it offers, but regardless it is mandated for now, because the Avalanche team does not currently have the tech capacity to offer a cloud-based offering. (This could change if we could raise a pile of money, but we aren't currently trying or planning on that right now.)

**Encrypted:** Pretty much all data in Avalanche's core is end-to-end encrypted, like Signal. It means that the server operator cannot read its users' data. This may sound like a drawback (and in some ways it does limit the power of the platform), but it gives you and your users a substantially reduced risk profile in the case that your server gets seized or hacked. This is a trade-off that we believe is ultimately worth it, and many organizers consider it mandatory.

### Bot and project security

The extensions to Avalanche (bots and projects) are in many ways a loophole in the security. So it's worth going into how that loophole works and what it means for your users. You don't have to make any tough decisions right away, but as your server grows you may want to be thinking about this:

**Bots** can be added to a group and can receive anything that gets sent in that group. If a bad bot gets added, that bot could record messages, even self-disappearing ones. (The same would be true of any unvetted human added to a Signal group -- they can always screenshot messages.)

We encourage bots not to record or log anything nonessential, but we cannot enforce it. As an administrator, you have the power to vet anyone, human or bot, before adding them to your server. 

Recall the seizure scenario though: the adversary shouldn't be able to read your users' messages. Well, if they seize a bot, then that's out the window: the adversary will read anything the bot can read. So your best defensive posture against seizure is to _separate the bots from the server itself_. You can run bots on the main server (this happens by default, since it's much easier) but we recommend as your server grows that bots are separately deployed. This is in fact quite easy to do, because bots can connect to the server from anywhere.

An **Adminbot** comes with nearly every Avalanche deployment and is an important bot to keep in mind. One of its purposes is to auto-add users to groups. That means it has to be in those groups, and that means it has access to all the messages, which means it is particularly sensitive. 

Other **projects** also may hold user data unencrypted. If you keep a user database with personally identifiable information, that user database is now a very attractive hacking or seizure target. The security posture for projects is similar to that for bots: as the administrator you approve projects that integrate with your Avalanche server. Like bots, projects can be separately hosted and deployed from the main server, and in most cases they should be as they scale.

**The upshot:** You don't have to stress about it right at the beginning, but as your server grows, it's worth thinking about how much data is within your seizure blast radius, and separating it out to protect your users' privacy.

# Bootstrap and administration

Ok, wanna get started? Let's get you set up.

You can host your Avalanche server anywhere that is open to the internet. If you're not sure where, try DigitalOcean. [Easy setup instructions are here](/configure/).[^defaultconfig]

[^defaultconfig]: The default configuration will start a server and an adminbot on the same machine. This is fine for small deployments, but larger and more security-sensitive deployments should move the adminbot to a different location.

### Onboarding flow

We recommend registering you and a couple of your co-organizers first. Chat and feel it out with them--the invite link + QR code in the easy setup instructions will work for co-administrators. Once you've added a few people, you'll likely want to start thinking about your onboarding flow before onboarding many more people. 

