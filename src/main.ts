// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { mount } from 'svelte';
import 'gridstack/dist/gridstack.min.css';
import './app.css';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app mount point');

mount(App, { target });
