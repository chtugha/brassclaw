// jsx-runtime symbols live inside the combined react bundle.
// Extract them so import { jsx } from "react/jsx-runtime" works.
import ReactModule from "react";
const internals = ReactModule.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;
export const jsx = internals?.jsx ?? ReactModule.createElement;
export const jsxs = internals?.jsxs ?? ReactModule.createElement;
export const jsxDEV = internals?.jsxDEV ?? ReactModule.createElement;
export const Fragment = ReactModule.Fragment;
export default ReactModule;
