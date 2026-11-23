# Changelog

## [0.4.0](https://github.com/Silvenga/lurklet/compare/v0.3.0...v0.4.0) (2026-11-23)


### Features

* added hosting of the openapi spec ([78cdf5e](https://github.com/Silvenga/lurklet/commit/78cdf5ee608e49ea4cda7d000bd8cf5ce5e6d41e))
* added logo to header ([93c46f2](https://github.com/Silvenga/lurklet/commit/93c46f2eb0e076154ebf45a5b3fdee2dae7b328e))
* added project icon ([e790e07](https://github.com/Silvenga/lurklet/commit/e790e077688a1daa4848c7c5c3ba79370900447d))
* added spoliers support ([5d3ce36](https://github.com/Silvenga/lurklet/commit/5d3ce3690ec0ca1dc8dbce04123105950eab510e))
* enabled tls profiles ([a5cf292](https://github.com/Silvenga/lurklet/commit/a5cf292446c3f59dfb11a528a8cb8238f7cecdef))
* improved/standarized error messages ([48f12c8](https://github.com/Silvenga/lurklet/commit/48f12c8a1ccab66f411e7efcb976ddd171ba08bb))
* updated comment ui ([88ec97e](https://github.com/Silvenga/lurklet/commit/88ec97e8d6cc321fbe7645d1f59b648cd95d71dc))


### Bug Fixes

* allowed proxying for more CDN domains ([c9d4603](https://github.com/Silvenga/lurklet/commit/c9d4603aeec83042a8271ab481fbad2ebc3bcdd1))
* fixed 403 when fetching video ([ae923bf](https://github.com/Silvenga/lurklet/commit/ae923bf076232f366258ad00590103c07e475b60))
* fixed detection of cached content through cdns ([369e7fe](https://github.com/Silvenga/lurklet/commit/369e7fe43a8c228827c0d9700b577c8b840461a1))
* reduced startup health convergence delay ([de20c13](https://github.com/Silvenga/lurklet/commit/de20c1339da13ddfa17a68267f70b78ccbbc0cdd))

## [0.3.0](https://github.com/Silvenga/lurklet/compare/v0.2.0...v0.3.0) (2026-11-04)


### Features

* added cache time display ([5796d01](https://github.com/Silvenga/lurklet/commit/5796d012c33a8e59e6788f8c63841d1840a85fbf))
* added cross-post handling ([316be9b](https://github.com/Silvenga/lurklet/commit/316be9ba9a4ae7a1cc6c20d3b42624ed29276360))
* added draft mp4 support ([35e1b9b](https://github.com/Silvenga/lurklet/commit/35e1b9b1cae7e6d971269b67facbb3ba164b4b76))
* added flairs rendering ([70fd5b1](https://github.com/Silvenga/lurklet/commit/70fd5b1310a2eb6b7a55646603fce22e4e61d3f1))
* added offline zstd compression support for static files ([484610b](https://github.com/Silvenga/lurklet/commit/484610b38e22b28099241dd09e1ea858ae31ef7e))
* hardened allowed query keys, added export-cache-query-keys verb ([bfb7b93](https://github.com/Silvenga/lurklet/commit/bfb7b93bb12fbd960374d8608ec65bf50c5823dd))
* improved cacheability of media ([9fc3fce](https://github.com/Silvenga/lurklet/commit/9fc3fcebb2640c9826bf1a7430b70d6af67701f0))
* separated media rate limits ([addd6d7](https://github.com/Silvenga/lurklet/commit/addd6d7fd9431e8c703864e94e3ff80911313079))
* switched "open in reddit" to use the upstream permalink ([bc25b28](https://github.com/Silvenga/lurklet/commit/bc25b28f70d31b973ce4fea984a0483df49ba28c))

## [0.2.0](https://github.com/Silvenga/lurklet/compare/v0.1.0...v0.2.0) (2026-11-01)


### Features

* added a "open in reddit" link ([d186e4c](https://github.com/Silvenga/lurklet/commit/d186e4ce85541e87ff153713e4fc0aa42339382d))
* added api static hosting ([29aca8c](https://github.com/Silvenga/lurklet/commit/29aca8ce958db7853241152e9e9d4352f3e7f1ce))
* added auth bootstrapping and testing cli ([79459f9](https://github.com/Silvenga/lurklet/commit/79459f92ccf0ec90020e385a11701367001eb018))
* added basic api file cache ([d5b5a28](https://github.com/Silvenga/lurklet/commit/d5b5a28eea4f73a34185fafece845bd8bf24b52d))
* added basic icons/thumbnails for subreddits and posts ([6bfb489](https://github.com/Silvenga/lurklet/commit/6bfb489c54da3d7e7a89eb87a77e9da862467ac3))
* added basic rate limiting ([5d2ca7b](https://github.com/Silvenga/lurklet/commit/5d2ca7b5ffaaf01b2cb7c0758ffd5598ee64a754))
* added comments endpoints ([a3307ca](https://github.com/Silvenga/lurklet/commit/a3307ca4701911fcba43028cad7d0aa052801662))
* added container image, added health check verb ([d3d654f](https://github.com/Silvenga/lurklet/commit/d3d654f7938d3dffd640cec9f2a8724ab9dafa0a))
* added image rendering ([60c9378](https://github.com/Silvenga/lurklet/commit/60c9378eab3bd100f7ae4e6aac01a9f0b1054361))
* added json link to api requests ([470c16b](https://github.com/Silvenga/lurklet/commit/470c16b480bbbb39a63b5167ae0b3020a3f081d2))
* added media proxying ([dc15ac8](https://github.com/Silvenga/lurklet/commit/dc15ac8a9f1b95e250da58ccaed08ac27de8874c))
* added opt-in state file ([9252389](https://github.com/Silvenga/lurklet/commit/9252389b9d18668f2e0459dccf3fddf482a67079))
* added post endpoint ([c335075](https://github.com/Silvenga/lurklet/commit/c335075bd1e6f2ce7d3a7d13e8960c9584c8f04c))
* added post list endpoint ([ce84c47](https://github.com/Silvenga/lurklet/commit/ce84c47a212d587146396fcb2a001ff3caa8ee51))
* added proxy support ([fb2fcd9](https://github.com/Silvenga/lurklet/commit/fb2fcd90b364709319882ccc85c2850573bc234d))
* added Referrer-Policy enforcement ([5b77c4c](https://github.com/Silvenga/lurklet/commit/5b77c4c997ba0c16f9a6081a4b80f5b64d149903))
* added response compression to api endpoints ([56bafe1](https://github.com/Silvenga/lurklet/commit/56bafe176cc94abffac91e9124803ea3503df184))
* added subreddit endpoint ([30074c8](https://github.com/Silvenga/lurklet/commit/30074c8402b4eeb4209bd4e41ea97e2dd18a325b))
* added wholesome suggested subreddits ([e358a93](https://github.com/Silvenga/lurklet/commit/e358a93ecc7118be886ee8d50eca122f53fd95ed))
* created a basic prototype of the ui ([5a2b13e](https://github.com/Silvenga/lurklet/commit/5a2b13e6b2afdb73e36cc6d8b901761edb56bafc))
* improved inline media rendering support, resolves [#28](https://github.com/Silvenga/lurklet/issues/28) [#27](https://github.com/Silvenga/lurklet/issues/27) ([800046a](https://github.com/Silvenga/lurklet/commit/800046ac2f8761f5d69d9d9d25645edaf3c0b757))
* improved reddit markdown support ([7a826e7](https://github.com/Silvenga/lurklet/commit/7a826e7e0c471d82ff3c5628ffd6a5164ecaa248))
