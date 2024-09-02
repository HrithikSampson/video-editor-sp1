use alloy_sol_types::sol;

sol! {
    /// The public values encoded as a struct that can be easily deserialized inside Solidity.
    struct PublicValuesStruct {
        &str base64_string;
        uint8 operation;
    }
}


use std::io::Read;

use image::RgbaImage;
use base64::{decode, encode};
use std::io::Cursor;
use ffmpeg_next::{codec, decoder, format, frame, software::scaling, util::dict};
use std::f64::consts::PI;

fn gaussian(x: f64) -> f64 {
    let mu: f64 = 0.0;
    let sigma: f64 = 1.0;
    let coefficient = 1.0 / (sigma * (2.0 * PI).sqrt());
    let exponent = -((x - mu).powi(2)) / (2.0 * sigma.powi(2));
    coefficient * exponent.exp()
}

fn adjust_brightness(frame: &mut RgbaImage, brightness: f32) {
    for pixel in frame.pixels_mut() {
        let channels = pixel.0;
        let r = (channels[0] as f32 * brightness).min(255.0) as u8;
        let g = (channels[1] as f32 * brightness).min(255.0) as u8;
        let b = (channels[2] as f32 * brightness).min(255.0) as u8;
        *pixel = image::Rgba([r, g, b, channels[3]]);
    }
}

fn invert_frame_horizontally(frame: &mut RgbaImage) {
    let (width, height) = frame.dimensions();
    for y in 0..height {
        for x in 0..width / 2 {
            let left_pixel = frame.get_pixel(x, y);
            let right_pixel = frame.get_pixel(width - x - 1, y);
            frame.put_pixel(x, y, *right_pixel);
            frame.put_pixel(width - x - 1, y, *left_pixel);
        }
    }
}

fn invert_frame_vertically(frame: &mut RgbaImage) {
    let (width, height) = frame.dimensions();
    for x in 0..width {
        for y in 0..height / 2 {
            let left_pixel = frame.get_pixel(x, y);
            let right_pixel = frame.get_pixel(x, height - y - 1);
            frame.put_pixel(x, y, *right_pixel);
            frame.put_pixel(x, height - y - 1, *left_pixel);
        }
    }
}

fn apply_shake_effect(frame: &mut RgbaImage, frame_index: u64) {
    let (width, height) = frame.dimensions();
    let shake_offset_x = (frame_index%2) ? 5: -5;
    let shake_offset_y = (frame_index%2) ? 5: -5;

    let mut shaken_img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let new_x = (x as i32 + shake_offset_x) % width as i32;
            let new_y = (y as i32 + shake_offset_y) % height as i32;
            if new_x >= 0 && new_y >= 0 && new_x < width as i32 && new_y < height as i32 {
                let pixel = frame.get_pixel(new_x as u32, new_y as u32);
                shaken_img.put_pixel(x, y, *pixel);
            }
        }
    }

    *frame = shaken_img;
}

fn apply_deformed_mirror_effect(frame: &mut RgbaImage) {
    let (width, height) = frame.dimensions();
    let mut mirrored_img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let new_x = if x < width / 2 {
                (width / 2 - x)
            } else {
                x - width / 2
            };
            let pixel = frame.get_pixel(new_x, y);
            mirrored_img.put_pixel(x, y, *pixel);
        }
    }

    *frame = mirrored_img;
}

fn process_video(operation: u8, base64_string: &str) -> String {
    let video_data = decode(base64_string).expect("Failed to decode base64 string");

    // Initialize FFmpeg and read the video data
    ffmpeg_next::init().unwrap();
    let mut input = format::input(&mut Cursor::new(video_data)).unwrap();
    let stream = input.streams().best(codec::media::Type::Video).unwrap();
    let video_stream_index = stream.index();
    let mut decoder = codec::context::Context::from_parameters(stream.parameters()).unwrap().decoder().video().unwrap();
    let mut scaler = scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg_next::format::Pixel::RGBA,
        decoder.width(),
        decoder.height(),
        scaling::flag::BILINEAR,
    ).unwrap();

    let mut frame_index: uint64 = 0;
    let mut output_frames = Vec::new();

    for (stream, packet) in input.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet).unwrap();

            let mut decoded = frame::Video::empty();

            let processed_frame = frame::Video::new(ffmpeg_next::format::Pixel::RGBA, decoder.width(), decoder.height());
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut rgba_frame = frame::Video::empty();
                scaler.run(&decoded, &mut rgba_frame).unwrap();

                let original_width = rgba_frame.width();
                let original_height = rgba_frame.height();
                let mut rgba_image = RgbaImage::from_raw(
                    original_width,
                    original_height,
                    rgba_frame.data(0).to_vec(),
                ).expect("Failed to create image from frame");

                match operation {
                    1 => {
                        adjust_brightness(&mut rgba_image, 1.2);
                        processed_frame.data_mut(0).copy_from_slice(rgba_image.as_raw());
                    },
                    2 => {
                        invert_frame_horizontally(&mut rgba_image)
                        processed_frame.data_mut(0).copy_from_slice(rgba_image.as_raw());
                    },
                    3 => {
                        invert_frame_vertically(&mut rgba_image);
                        processed_frame.data_mut(0).copy_from_slice(rgba_image.as_raw());
                    },
                    4 => {
                        apply_shake_effect(&mut rgba_image, frame_index);
                        processed_frame.data_mut(0).copy_from_slice(rgba_image.as_raw());
                    },
                    5 => {
                        apply_deformed_mirror_effect(&mut rgba_image)

                        //TODO: resize-logic - possibly due to the nature
                        let resized_image = image::imageops::resize(
                            &rgba_image,
                            original_width,
                            original_height,
                            image::imageops::FilterType::Lanczos3,
                        );
                        processed_frame.data_mut(0).copy_from_slice(resized_image.as_raw());
                    }
                }
                // TODO: Convert the processed frame back to a video frame
                output_frames.push(processed_frame);
                frame_index += 1;
            }
        }
    }

    // TODO: Reassemble the frames into a video and encode it
    let processed_video_base64 = encode("processed video data");

    processed_video_base64
}
