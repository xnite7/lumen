// The landing page and install guide (designed in Claude Design): the pages plus their pictures and font.
import landing from "./index.html";
import install from "./install.html";
import a0 from "./amb-body.png";
import a1 from "./amb-cal.png";
import a2 from "./amb-scrubs.png";
import a3 from "./amb-smart.png";
import a4 from "./amb-upnext.png";
import a5 from "./av-lx0nit.jpg";
import a6 from "./av-mari.jpg";
import a7 from "./av-north.jpg";
import a8 from "./av-xnite.jpg";
import a9 from "./couch-boil.webp";
import a10 from "./grain.png";
import a11 from "./hero-body.jpg";
import a12 from "./hero-scrubs.jpg";
import a13 from "./logo-body.png";
import a14 from "./logo-scrubs.png";
import a15 from "./satoshi.woff2";
import a16 from "./scr-cal.jpg";
import a17 from "./scr-couch.jpg";
import a18 from "./scr-lists.jpg";
import a19 from "./scr-pip.jpg";
import a20 from "./scr-smart.jpg";
import a21 from "./scr-upnext.jpg";
import a22 from "./th-a.jpg";
import a23 from "./th-b.jpg";
import a24 from "./th-c.jpg";

export const LANDING_HTML: string = landing;
export const INSTALL_PAGE: string = install;
export const LANDING_FILES: Record<string, [ArrayBuffer, string]> = {
  "amb-body.png": [a0, "image/png"],
  "amb-cal.png": [a1, "image/png"],
  "amb-scrubs.png": [a2, "image/png"],
  "amb-smart.png": [a3, "image/png"],
  "amb-upnext.png": [a4, "image/png"],
  "av-lx0nit.jpg": [a5, "image/jpeg"],
  "av-mari.jpg": [a6, "image/jpeg"],
  "av-north.jpg": [a7, "image/jpeg"],
  "av-xnite.jpg": [a8, "image/jpeg"],
  "couch-boil.webp": [a9, "image/webp"],
  "grain.png": [a10, "image/png"],
  "hero-body.jpg": [a11, "image/jpeg"],
  "hero-scrubs.jpg": [a12, "image/jpeg"],
  "logo-body.png": [a13, "image/png"],
  "logo-scrubs.png": [a14, "image/png"],
  "satoshi.woff2": [a15, "font/woff2"],
  "scr-cal.jpg": [a16, "image/jpeg"],
  "scr-couch.jpg": [a17, "image/jpeg"],
  "scr-lists.jpg": [a18, "image/jpeg"],
  "scr-pip.jpg": [a19, "image/jpeg"],
  "scr-smart.jpg": [a20, "image/jpeg"],
  "scr-upnext.jpg": [a21, "image/jpeg"],
  "th-a.jpg": [a22, "image/jpeg"],
  "th-b.jpg": [a23, "image/jpeg"],
  "th-c.jpg": [a24, "image/jpeg"],
};
