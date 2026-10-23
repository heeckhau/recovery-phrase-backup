// Allow the inline SVG animation, and keep emoji as text so the HTML doesn't
// load Twemoji images from a CDN.
module.exports = {
  html: true,
  options: { emoji: { shortcode: false, unicode: false } },
};
