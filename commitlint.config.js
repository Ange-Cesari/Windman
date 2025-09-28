/** Conventional Commits rules */
module.exports = {
    extends: ['@commitlint/config-conventional'],
    // (optional) relax/adjust rules here
    rules: {
      // allow any case in subject (feel free to tighten later)
      'subject-case': [0],
    },
  };