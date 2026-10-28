---
layout: post
title: "First Post"
date: 2020-05-01 09:00:00 +0200
tags: [jekyll, migration]
image: /assets/img/cat.png
description: The very first post.
author: Jay Kyll
---
Hello from Jekyll. See the [second post]({% post_url 2020-06-01-second %}) and
[about]({% link about.md %}).

![Cat]({{ site.baseurl }}/assets/img/cat.png)

{% highlight ruby %}
puts "hi"
{% endhighlight %}

{% raw %}Literal {{ liquid }} stays.{% endraw %}

{% include note.html %}
