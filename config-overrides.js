// config-overrides.js
const { override, addWebpackModuleRule } = require('customize-cra');

module.exports = override(
  addWebpackModuleRule({
    test: /\.worker\.js$/,
    use: { loader: 'worker-loader' }
  })
);