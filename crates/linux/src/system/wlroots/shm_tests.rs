use super::*;

fn spec(width: u32, height: u32, stride: u32) -> BufferSpec {
    BufferSpec {
        width,
        height,
        stride,
        format: 0,
    }
}

#[test]
fn a_buffer_is_its_stride_times_its_height() {
    assert_eq!(byte_length(spec(4, 3, 16)).expect("fits"), 48);
    assert_eq!(byte_length(spec(4, 3, 20)).expect("padded"), 60);
}

#[test]
fn an_empty_or_narrow_buffer_is_refused() {
    assert!(byte_length(spec(0, 3, 16)).is_err());
    assert!(byte_length(spec(4, 3, 15)).is_err());
}
