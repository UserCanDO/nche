import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "export",
  // Disable image optimization for static export
  images: {
    unoptimized: true,
  },
  // Trailing slashes help with static file serving
  trailingSlash: true,
};

export default nextConfig;
