// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2022 Adrian <adrian.eddy at gmail>

use wgpu::TextureFormat;
use wgpu_hal::api::Vulkan;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Foundation::{ CloseHandle, HANDLE };
use windows::core::Interface;
use ash::vk::{self, ImageCreateInfo};

// https://github.com/artumino/VRScreenCap/blob/main/src/loaders/katanga_loader.rs

pub fn get_shared_texture_d3d11(device: &ID3D11Device, texture: &ID3D11Texture2D) -> Result<(HANDLE, ID3D11Texture2D, bool), Box<dyn std::error::Error>> { // (shared handle, texture, is_new_texture)
    unsafe {
        // Try to open or create shared handle if possible
        if let Ok(dxgi_resource) = texture.cast::<IDXGIResource1>() {
            if let Ok(handle) = dxgi_resource.CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE, None) {
                if handle.0 > 0 {
                    return Ok((handle, texture.clone(), false));
                }
            }
        }

        // No shared handle and not possible to create one.
        // We need to create a new texture and use texture copy from our original one.
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX;

        let new_texture = device.CreateTexture2D(&desc, None)?;
        let dxgi_resource: IDXGIResource1 = new_texture.cast::<IDXGIResource1>()?;
        let handle = dxgi_resource.CreateSharedHandle(None, DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE, None)?;

        Ok((handle, new_texture, true))
    }
}

pub fn create_vk_image_from_d3d11_texture(device: &wgpu::Device, d3d11_device: &ID3D11Device, texture: &ID3D11Texture2D) -> Result<(vk::Image, Option<ID3D11Texture2D>), Box<dyn std::error::Error>> {
    unsafe {
        let (handle, texture, is_new) = get_shared_texture_d3d11(d3d11_device, texture).unwrap(); // TODO: unwrap

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);

        assert_eq!(desc.SampleDesc.Count, 1);
        assert_eq!(desc.MipLevels, 1);
        assert_eq!(desc.ArraySize, 1);

        let raw_image = device.as_hal::<Vulkan, _, _>(|device| {
            device.map(|device| {
                let raw_device = device.raw_device();
                let handle_type = vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE; // D3D12_RESOURCE_KHR

                let mut import_memory_info = vk::ImportMemoryWin32HandleInfoKHR::builder()
                    .handle_type(handle_type)
                    .handle(handle.0 as *mut std::ffi::c_void);

                let allocate_info = vk::MemoryAllocateInfo::builder()
                    .push_next(&mut import_memory_info)
                    .memory_type_index(0);

                let allocated_memory = raw_device.allocate_memory(&allocate_info, None)?;

                let mut ext_create_info = vk::ExternalMemoryImageCreateInfo::builder().handle_types(handle_type);

                let image_create_info = ImageCreateInfo::builder()
                    .push_next(&mut ext_create_info)
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(super::wgpu_interop_vulkan::format_wgpu_to_vulkan(format_dxgi_to_wgpu(desc.Format)))
                    .extent(vk::Extent3D {
                        width: desc.Width,
                        height: desc.Height,
                        depth: desc.ArraySize,
                    })
                    .mip_levels(desc.MipLevels)
                    .array_layers(desc.ArraySize)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let raw_image = raw_device.create_image(&image_create_info, None)?;

                raw_device.bind_image_memory(raw_image, allocated_memory, 0)?;

                CloseHandle(handle);

                Ok::<ash::vk::Image, vk::Result>(raw_image)
            })
        }).unwrap()?; // TODO: unwrap

        Ok((raw_image, if is_new { Some(texture) } else { None }))
    }
}


pub fn format_dxgi_to_wgpu(format: DXGI_FORMAT) -> TextureFormat {
    match format {
        DXGI_FORMAT_R8_UNORM => TextureFormat::R8Unorm,
        DXGI_FORMAT_R8_SNORM => TextureFormat::R8Snorm,
        DXGI_FORMAT_R8_UINT => TextureFormat::R8Uint,
        DXGI_FORMAT_R8_SINT => TextureFormat::R8Sint,
        DXGI_FORMAT_R16_UINT => TextureFormat::R16Uint,
        DXGI_FORMAT_R16_SINT => TextureFormat::R16Sint,
        DXGI_FORMAT_R16_UNORM => TextureFormat::R16Unorm,
        DXGI_FORMAT_R16_SNORM => TextureFormat::R16Snorm,
        DXGI_FORMAT_R16_FLOAT => TextureFormat::R16Float,
        DXGI_FORMAT_R8G8_UNORM => TextureFormat::Rg8Unorm,
        DXGI_FORMAT_R8G8_SNORM => TextureFormat::Rg8Snorm,
        DXGI_FORMAT_R8G8_UINT => TextureFormat::Rg8Uint,
        DXGI_FORMAT_R8G8_SINT => TextureFormat::Rg8Sint,
        DXGI_FORMAT_R16G16_UNORM => TextureFormat::Rg16Unorm,
        DXGI_FORMAT_R16G16_SNORM => TextureFormat::Rg16Snorm,
        DXGI_FORMAT_R32_UINT => TextureFormat::R32Uint,
        DXGI_FORMAT_R32_SINT => TextureFormat::R32Sint,
        DXGI_FORMAT_R32_FLOAT => TextureFormat::R32Float,
        DXGI_FORMAT_R16G16_UINT => TextureFormat::Rg16Uint,
        DXGI_FORMAT_R16G16_SINT => TextureFormat::Rg16Sint,
        DXGI_FORMAT_R16G16_FLOAT => TextureFormat::Rg16Float,
        DXGI_FORMAT_R8G8B8A8_TYPELESS => TextureFormat::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM => TextureFormat::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => TextureFormat::Rgba8UnormSrgb,
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => TextureFormat::Bgra8UnormSrgb,
        DXGI_FORMAT_R8G8B8A8_SNORM => TextureFormat::Rgba8Snorm,
        DXGI_FORMAT_B8G8R8A8_UNORM => TextureFormat::Bgra8Unorm,
        DXGI_FORMAT_R8G8B8A8_UINT => TextureFormat::Rgba8Uint,
        DXGI_FORMAT_R8G8B8A8_SINT => TextureFormat::Rgba8Sint,
        DXGI_FORMAT_R10G10B10A2_UNORM => TextureFormat::Rgb10a2Unorm,
        DXGI_FORMAT_R11G11B10_FLOAT => TextureFormat::Rg11b10Float,
        DXGI_FORMAT_R32G32_UINT => TextureFormat::Rg32Uint,
        DXGI_FORMAT_R32G32_SINT => TextureFormat::Rg32Sint,
        DXGI_FORMAT_R32G32_FLOAT => TextureFormat::Rg32Float,
        DXGI_FORMAT_R16G16B16A16_UINT => TextureFormat::Rgba16Uint,
        DXGI_FORMAT_R16G16B16A16_SINT => TextureFormat::Rgba16Sint,
        DXGI_FORMAT_R16G16B16A16_UNORM => TextureFormat::Rgba16Unorm,
        DXGI_FORMAT_R16G16B16A16_SNORM => TextureFormat::Rgba16Snorm,
        DXGI_FORMAT_R16G16B16A16_FLOAT => TextureFormat::Rgba16Float,
        DXGI_FORMAT_R32G32B32A32_UINT => TextureFormat::Rgba32Uint,
        DXGI_FORMAT_R32G32B32A32_SINT => TextureFormat::Rgba32Sint,
        DXGI_FORMAT_R32G32B32A32_FLOAT => TextureFormat::Rgba32Float,
        DXGI_FORMAT_D32_FLOAT => TextureFormat::Depth32Float,
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT => TextureFormat::Depth32FloatStencil8,
        DXGI_FORMAT_R9G9B9E5_SHAREDEXP => TextureFormat::Rgb9e5Ufloat,
        DXGI_FORMAT_BC1_UNORM => TextureFormat::Bc1RgbaUnorm,
        DXGI_FORMAT_BC1_UNORM_SRGB => TextureFormat::Bc1RgbaUnormSrgb,
        DXGI_FORMAT_BC2_UNORM => TextureFormat::Bc2RgbaUnorm,
        DXGI_FORMAT_BC2_UNORM_SRGB => TextureFormat::Bc2RgbaUnormSrgb,
        DXGI_FORMAT_BC3_UNORM => TextureFormat::Bc3RgbaUnorm,
        DXGI_FORMAT_BC3_UNORM_SRGB => TextureFormat::Bc3RgbaUnormSrgb,
        DXGI_FORMAT_BC4_UNORM => TextureFormat::Bc4RUnorm,
        DXGI_FORMAT_BC4_SNORM => TextureFormat::Bc4RSnorm,
        DXGI_FORMAT_BC5_UNORM => TextureFormat::Bc5RgUnorm,
        DXGI_FORMAT_BC5_SNORM => TextureFormat::Bc5RgSnorm,
        DXGI_FORMAT_BC6H_UF16 => TextureFormat::Bc6hRgbUfloat,
        DXGI_FORMAT_BC6H_SF16 => TextureFormat::Bc6hRgbSfloat,
        DXGI_FORMAT_BC7_UNORM => TextureFormat::Bc7RgbaUnorm,
        DXGI_FORMAT_BC7_UNORM_SRGB => TextureFormat::Bc7RgbaUnormSrgb,
        _ => panic!("Unsupported texture format: {:?}", format),
    }
}
