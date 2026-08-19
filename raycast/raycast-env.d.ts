/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** Day Start Hour - Logical day starts at this local hour (0-23). Late-night work counts as the prior day. */
  "day-start-hour": string
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `today` command */
  export type Today = ExtensionPreferences & {}
  /** Preferences accessible in the `inbox` command */
  export type Inbox = ExtensionPreferences & {}
  /** Preferences accessible in the `review` command */
  export type Review = ExtensionPreferences & {}
  /** Preferences accessible in the `quick-create` command */
  export type QuickCreate = ExtensionPreferences & {}
  /** Preferences accessible in the `menubar` command */
  export type Menubar = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `today` command */
  export type Today = {}
  /** Arguments passed to the `inbox` command */
  export type Inbox = {}
  /** Arguments passed to the `review` command */
  export type Review = {}
  /** Arguments passed to the `quick-create` command */
  export type QuickCreate = {}
  /** Arguments passed to the `menubar` command */
  export type Menubar = {}
}

