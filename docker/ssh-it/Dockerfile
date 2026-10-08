FROM alpine:3.20

RUN apk add --no-cache openssh-server openssh-keygen shadow su-exec

RUN adduser -D -u 1000 wisp && echo 'wisp:wisp' | chpasswd

COPY sshd_config /etc/ssh/sshd_config
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh && ssh-keygen -A

EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
