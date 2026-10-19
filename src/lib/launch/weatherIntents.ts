// The weathers Pit Box offers, by the intent the player picks (`weather.rs`,
// `INTENTS`): the ids are the backend's, the labels the app's. One list for
// every screen that names a weather, so a new intent is added once.

export const WEATHER_IDS = ["clear", "few_clouds", "overcast", "fog", "light_rain", "rain", "storm", "snow"] as const;

export const WEATHER_LABEL_KEYS: Record<string, string> = {
  clear: "launch.wxClear",
  few_clouds: "launch.wxFewClouds",
  overcast: "launch.wxOvercast",
  fog: "launch.wxFog",
  light_rain: "launch.wxLightRain",
  rain: "launch.wxRain",
  storm: "launch.wxStorm",
  snow: "launch.wxSnow",
};
