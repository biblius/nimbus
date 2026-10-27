# Introduction

Nimbus is a file hosting application (think Google Drive at home).

This manual will equip you with the knowledge to set up your own instance
in no time. The manual is focused on bare-metal deployments, but the
same principles apply if you are using Docker.

If you know what Nextcloud is then Nimbus needs no further introduction,
you can head on straight to the next chapter of this manual to set it up.

## Why Nimbus?

At the time of writing this guide (26.10.2026.), Nimbus is a 1 man endeavour.

Nextcloud has suited my needs for a long time, but I am afraid it is time to
move on. I am running this thing on an Orange PI Zero 3, it is taking forever
on the initial load and the update process is megajank.

Keeping Nextcloud up to date involves keeping your PHP (lol) version up to date and requires
you to update incrementally, so you can't just go from version 69 to version 420.
After getting a bunch of permission and missing script errors, I realised way
too much of my time is going into updating what should basically be a drag and drop.

After looking at alternatives - and seeing they all had a "Try it now" and "Solutions"
on their main pages - I said fuck it, we'll do it live.

**Nimbus's main goal is, and always will be, to provide a simple and
effective way to host and share files**, because I want it to be fast
and secure.

**Another goal is to keep the configuration and update process for Nimbus as
simple as possible**, because I value my time and yours.
