# Architecture Guidelines

This document describes architectural patterns and conventions used in this codebase. AI coding agents and contributors should follow these guidelines when making changes.

## Database Schema

All MongoDB collections, indexes, and TTLs are defined centrally in `backend/app/data/schema.py`.
