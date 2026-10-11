module.exports = function(eleventyConfig) {
  return {
    dir: {
      input: ".",
      output: "_site",
      includes: "_includes"
    },
    markdownTemplateEngine: false
  };
};
